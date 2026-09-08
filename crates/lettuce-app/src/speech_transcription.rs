use std::time::Duration;

use lettuce_jobs::{
    CancellationPolicy, CancellationReason, Claim, FiniteFraction, IdempotencyKey, JobError,
    JobErrorCode, JobKind, JobMutation, JobOutcome, JobPriority, JobSnapshot, JobState, JobStore,
    JobSubject, OutcomeRef, ProgressSnapshot, RecoveryPolicy, ResourceAvailability, ResourceClass,
    StageSnapshot, StoreError, SubjectKind, WorkerId, handle::JobHandle,
};
use lettuce_speech::{
    AsrAudioError, AsrAudioSource, AsrLibraryError, AsrPromptLibrary, AsrRuntime, AsrRuntimeError,
    AsrValidationError, TranscriptionRecord, TranscriptionRepository, TranscriptionRepositoryError,
    TranscriptionRequest, TranscriptionResult, TranscriptionState, merge_transcription_prompt,
};
use lettuce_types::{JobId, TimestampMillis};

#[derive(Debug, Clone, PartialEq)]
pub struct SpeechTranscriptionAdmission {
    pub record: TranscriptionRecord,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug)]
pub struct SpeechTranscriptionClaimedWork {
    pub record: TranscriptionRecord,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeechTranscriptionSuccess {
    pub record: TranscriptionRecord,
    pub job: JobSnapshot,
    pub replayed: bool,
}

#[derive(Debug)]
pub enum SpeechTranscriptionRunResult {
    Succeeded(Box<SpeechTranscriptionSuccess>),
    Cancelled {
        error: SpeechTranscriptionError,
        job: JobSnapshot,
    },
    Failed {
        error: SpeechTranscriptionError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: SpeechTranscriptionError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum SpeechTranscriptionError {
    #[error("transcription request is invalid: {0}")]
    Invalid(#[from] AsrValidationError),
    #[error("transcription audio failed: {0}")]
    Audio(#[from] AsrAudioError),
    #[error("transcription learning library failed: {0}")]
    Library(#[from] AsrLibraryError),
    #[error("transcription runtime failed: {0}")]
    Runtime(#[from] AsrRuntimeError),
    #[error("transcription persistence failed: {0}")]
    Repository(#[from] TranscriptionRepositoryError),
    #[error("transcription job failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("transcription job and request are inconsistent")]
    InvalidWork,
}

#[derive(Debug)]
pub struct SpeechTranscriptionCoordinator<'a, R: ?Sized, J: ?Sized> {
    transcriptions: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> SpeechTranscriptionCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(transcriptions: &'a R, jobs: &'a J) -> Self {
        Self {
            transcriptions,
            jobs,
        }
    }
}

impl<R: TranscriptionRepository + ?Sized, J: JobStore + ?Sized>
    SpeechTranscriptionCoordinator<'_, R, J>
{
    pub fn admit(
        &self,
        request: TranscriptionRequest,
    ) -> Result<SpeechTranscriptionAdmission, SpeechTranscriptionError> {
        request.validate()?;
        let subject = JobSubject::new(SubjectKind::SpeechRequest, request.id.to_string())
            .map_err(|_| SpeechTranscriptionError::InvalidWork)?;
        let key = IdempotencyKey::new(format!("speech-transcribe-{}", request.id))
            .map_err(|_| SpeechTranscriptionError::InvalidWork)?;
        let mut resources = vec![
            ResourceClass::DiskRead,
            ResourceClass::ModelLoad,
            ResourceClass::Cpu,
            ResourceClass::DiskWrite,
        ];
        if request.options.use_gpu {
            resources.push(ResourceClass::Gpu);
        }
        let admitted = self.jobs.create_or_get(
            lettuce_jobs::JobSpec::new(
                JobKind::SpeechTranscribe,
                subject,
                OutcomeRef::Request(request.id),
            )
            .with_idempotency_key(key)
            .with_priority(JobPriority::Interactive)
            .with_resources(resources)
            .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative),
        )?;
        let record = self.transcriptions.admit(TranscriptionRecord {
            job_id: admitted.job.id,
            request,
            state: TranscriptionState::Pending,
        })?;
        validate_job_record(&admitted.job, &record)?;
        Ok(SpeechTranscriptionAdmission {
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
    ) -> Result<Option<SpeechTranscriptionClaimedWork>, SpeechTranscriptionError> {
        let record = self.transcriptions.get(job_id)?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(SpeechTranscriptionError::InvalidWork)?;
        validate_job_record(&job, &record)?;
        if job.state == JobState::Succeeded {
            return Ok(None);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self.jobs.claim(job_id, worker_id, at, lease_for, allowed)? else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::Request(record.request.id) {
            return Err(SpeechTranscriptionError::InvalidWork);
        }
        let handle = JobHandle::new(job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("speech-transcription", false)
                .expect("constant stage is valid"),
            at,
        })?;
        Ok(Some(SpeechTranscriptionClaimedWork {
            record,
            claim,
            handle,
            job,
        }))
    }

    pub fn replay(
        &self,
        job_id: JobId,
    ) -> Result<Option<SpeechTranscriptionSuccess>, SpeechTranscriptionError> {
        let record = self.transcriptions.get(job_id)?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(SpeechTranscriptionError::InvalidWork)?;
        validate_job_record(&job, &record)?;
        match (&record.state, job.state) {
            (TranscriptionState::Succeeded { .. }, JobState::Succeeded) => {
                Ok(Some(SpeechTranscriptionSuccess {
                    record,
                    job,
                    replayed: true,
                }))
            }
            _ => Ok(None),
        }
    }

    pub fn run<A: AsrAudioSource + ?Sized, L: AsrPromptLibrary + ?Sized, E: AsrRuntime + ?Sized>(
        &self,
        work: SpeechTranscriptionClaimedWork,
        audio: &A,
        library: &L,
        runtime: &E,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<SpeechTranscriptionRunResult, SpeechTranscriptionError> {
        validate_job_record(&work.job, &work.record)?;
        if work.job.state != JobState::Running
            || work.claim.claim.job_id != work.job.id
            || work.handle.id() != work.job.id
        {
            return Err(SpeechTranscriptionError::InvalidWork);
        }
        if let TranscriptionState::Succeeded { .. } = work.record.state {
            return self.finish_success(work, true, now);
        }
        let result = self.execute(&work, audio, library, runtime, now);
        match result {
            Ok(result) => match self.transcriptions.settle(work.job.id, result) {
                Ok(record) => self.finish_success(
                    SpeechTranscriptionClaimedWork { record, ..work },
                    false,
                    now,
                ),
                Err(error) => self.finish_error(work, error.into(), now),
            },
            Err(error)
                if matches!(
                    error,
                    SpeechTranscriptionError::Runtime(AsrRuntimeError::Cancelled)
                ) || work.handle.cancellation_token().is_cancelled() =>
            {
                let at = now.max(work.job.updated_at);
                self.jobs
                    .append_and_transition(JobMutation::RequestCancellation {
                        id: work.job.id,
                        reason: cancellation_reason,
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
                Ok(SpeechTranscriptionRunResult::Cancelled { error, job })
            }
            Err(error) => self.finish_error(work, error, now),
        }
    }

    fn execute<A: AsrAudioSource + ?Sized, L: AsrPromptLibrary + ?Sized, E: AsrRuntime + ?Sized>(
        &self,
        work: &SpeechTranscriptionClaimedWork,
        audio: &A,
        library: &L,
        runtime: &E,
        now: TimestampMillis,
    ) -> Result<TranscriptionResult, SpeechTranscriptionError> {
        let token = work.handle.cancellation_token();
        check_cancelled(&token)?;
        let decoded = audio.decode(work.record.request.audio_asset_id)?;
        let samples = decoded.mono_16khz()?;
        check_cancelled(&token)?;
        let vocabulary_prompt = library.build_prompt(
            work.record.request.options.language.as_deref(),
            &work.record.request.options.scopes,
        )?;
        let prompt = merge_transcription_prompt(
            &vocabulary_prompt,
            work.record.request.options.initial_prompt.as_deref(),
        )?;
        let runtime_result = runtime.transcribe(
            &work.record.request.model,
            &samples,
            &prompt,
            &work.record.request.options,
            &token,
        )?;
        runtime_result.validate()?;
        check_cancelled(&token)?;
        let (corrected_text, applied_corrections) = library.apply_corrections(
            &runtime_result.raw_text,
            work.record.request.options.language.as_deref(),
            &work.record.request.options.scopes,
        )?;
        let result = TranscriptionResult {
            request_id: work.record.request.id,
            audio_asset_id: work.record.request.audio_asset_id,
            model: work.record.request.model.clone(),
            sample_rate_hz: 16_000,
            prompt,
            raw_text: runtime_result.raw_text,
            corrected_text,
            detected_language: runtime_result.detected_language,
            segments: runtime_result.segments,
            applied_corrections,
            completed_at: now,
        };
        result.validate_for(&work.record.request)?;
        Ok(result)
    }

    fn finish_success(
        &self,
        work: SpeechTranscriptionClaimedWork,
        replayed: bool,
        now: TimestampMillis,
    ) -> Result<SpeechTranscriptionRunResult, SpeechTranscriptionError> {
        if !matches!(work.record.state, TranscriptionState::Succeeded { .. }) {
            return Err(SpeechTranscriptionError::InvalidWork);
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
        Ok(SpeechTranscriptionRunResult::Succeeded(Box::new(
            SpeechTranscriptionSuccess {
                record: work.record,
                job,
                replayed,
            },
        )))
    }

    fn finish_error(
        &self,
        work: SpeechTranscriptionClaimedWork,
        error: SpeechTranscriptionError,
        now: TimestampMillis,
    ) -> Result<SpeechTranscriptionRunResult, SpeechTranscriptionError> {
        let at = now.max(work.job.updated_at);
        let (code, retryable, message) = classify_error(&error);
        if retryable {
            let job = self
                .jobs
                .append_and_transition(JobMutation::RetryScheduled {
                    claim: work.claim.claim,
                    at,
                })?;
            return Ok(SpeechTranscriptionRunResult::RetryScheduled { error, job });
        }
        let job = self.jobs.append_and_transition(JobMutation::Fail {
            claim: work.claim.claim,
            error: JobError::new(code, false, message)
                .expect("constant speech error label is valid"),
            at,
        })?;
        Ok(SpeechTranscriptionRunResult::Failed { error, job })
    }
}

fn validate_job_record(
    job: &JobSnapshot,
    record: &TranscriptionRecord,
) -> Result<(), SpeechTranscriptionError> {
    record.validate()?;
    if job.id != record.job_id
        || job.kind != JobKind::SpeechTranscribe
        || job.subject.kind != SubjectKind::SpeechRequest
        || job.subject.id.as_str() != record.request.id.to_string()
    {
        return Err(SpeechTranscriptionError::InvalidWork);
    }
    if job.state == JobState::Succeeded
        && (!matches!(record.state, TranscriptionState::Succeeded { .. })
            || job.outcome.as_ref()
                != Some(&JobOutcome::Success {
                    result_ref: OutcomeRef::Request(record.request.id),
                }))
    {
        return Err(SpeechTranscriptionError::InvalidWork);
    }
    Ok(())
}

fn check_cancelled(
    token: &lettuce_jobs::handle::CancellationToken,
) -> Result<(), SpeechTranscriptionError> {
    if token.is_cancelled() {
        Err(AsrRuntimeError::Cancelled.into())
    } else {
        Ok(())
    }
}

fn classify_error(error: &SpeechTranscriptionError) -> (JobErrorCode, bool, &'static str) {
    match error {
        SpeechTranscriptionError::Invalid(_)
        | SpeechTranscriptionError::Audio(
            AsrAudioError::InvalidAudio
            | AsrAudioError::UnsupportedFormat
            | AsrAudioError::TooLarge,
        )
        | SpeechTranscriptionError::Library(AsrLibraryError::InvalidData)
        | SpeechTranscriptionError::Runtime(AsrRuntimeError::Rejected)
        | SpeechTranscriptionError::InvalidWork => (
            JobErrorCode::InvalidInput,
            false,
            "speech transcription input is invalid",
        ),
        SpeechTranscriptionError::Runtime(AsrRuntimeError::ModelUnavailable) => (
            JobErrorCode::CapabilityUnavailable,
            true,
            "speech model is unavailable",
        ),
        SpeechTranscriptionError::Runtime(AsrRuntimeError::Cancelled) => (
            JobErrorCode::Cancelled,
            false,
            "speech transcription was cancelled",
        ),
        SpeechTranscriptionError::Audio(_)
        | SpeechTranscriptionError::Library(_)
        | SpeechTranscriptionError::Runtime(_)
        | SpeechTranscriptionError::Repository(_)
        | SpeechTranscriptionError::Jobs(_) => (
            JobErrorCode::ResourceUnavailable,
            true,
            "speech transcription resource is unavailable",
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use lettuce_database::Database;
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
        MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
    };
    use lettuce_speech::{
        AppliedCorrection, AsrModelDescriptor, AsrModelId, DecodedAudio, RuntimeTranscription,
        TranscriptionOptions, TranscriptionSegment,
    };
    use lettuce_types::{AssetId, ContentHash, MediaBlobId, RequestId, Revision, TimestampMillis};

    use super::*;

    const NOW: TimestampMillis = TimestampMillis::new(1_000);

    #[derive(Debug)]
    struct Audio;

    impl AsrAudioSource for Audio {
        fn decode(&self, _: AssetId) -> Result<DecodedAudio, AsrAudioError> {
            Ok(DecodedAudio {
                samples: vec![0.25, -0.25, 0.5, 0.5],
                sample_rate_hz: 8_000,
                channels: 2,
            })
        }
    }

    #[derive(Debug)]
    struct Library;

    impl AsrPromptLibrary for Library {
        fn build_prompt(&self, _: Option<&str>, _: &[String]) -> Result<String, AsrLibraryError> {
            Ok("Lettuce AI".into())
        }

        fn apply_corrections(
            &self,
            text: &str,
            _: Option<&str>,
            _: &[String],
        ) -> Result<(String, Vec<AppliedCorrection>), AsrLibraryError> {
            Ok((
                text.replace("lettuce a eye", "Lettuce AI"),
                vec![AppliedCorrection {
                    correction_id: "correction-1".into(),
                    wrong: "lettuce a eye".into(),
                    correct: "Lettuce AI".into(),
                    matched_text: "lettuce a eye".into(),
                }],
            ))
        }
    }

    #[derive(Debug)]
    struct Runtime {
        calls: Arc<Mutex<u32>>,
        outcome: Result<(), AsrRuntimeError>,
    }

    impl AsrRuntime for Runtime {
        fn transcribe(
            &self,
            _: &AsrModelDescriptor,
            samples: &[f32],
            prompt: &str,
            _: &TranscriptionOptions,
            cancellation: &lettuce_jobs::handle::CancellationToken,
        ) -> Result<RuntimeTranscription, AsrRuntimeError> {
            *self.calls.lock().expect("calls") += 1;
            assert_eq!(samples, [0.0, 0.25, 0.5, 0.5]);
            assert_eq!(prompt, "Lettuce AI custom prompt");
            if cancellation.is_cancelled() {
                return Err(AsrRuntimeError::Cancelled);
            }
            self.outcome?;
            Ok(RuntimeTranscription {
                raw_text: "lettuce a eye works".into(),
                detected_language: Some("en".into()),
                segments: vec![TranscriptionSegment {
                    index: 0,
                    start_ms: 0,
                    end_ms: 250,
                    text: "lettuce a eye works".into(),
                    no_speech_probability: 0.1,
                    speaker_turn_next: false,
                }],
            })
        }
    }

    fn insert_audio(database: &Database) -> AssetId {
        let blob = MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse("ab".repeat(32)).expect("hash"),
            kind: MediaKind::Audio,
            mime_type: "audio/wav".into(),
            byte_size: 44,
            width: None,
            height: None,
            duration_ms: Some(250),
            validation_version: 1,
            state: BlobState::Staged,
            created_at: NOW,
            updated_at: NOW,
        };
        let blob = MediaBlobRepository::register(database, blob).expect("blob");
        MediaBlobRepository::finalize_staged_to_ready(database, blob.id, NOW).expect("ready blob");
        let asset = MediaAsset::new(
            AssetId::new(),
            blob.id,
            AssetKind::MessageAudio,
            AssetOrigin::Upload,
            RetentionClass::Temporary {
                expires_at: TimestampMillis::new(10_000),
            },
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            NOW,
            NOW,
        )
        .expect("asset");
        MediaAssetRepository::create(database, asset.clone()).expect("stored asset");
        asset.id
    }

    fn request(audio_asset_id: AssetId) -> TranscriptionRequest {
        TranscriptionRequest {
            id: RequestId::new(),
            audio_asset_id,
            model: AsrModelDescriptor {
                id: AsrModelId::new("small.en-q5_1").expect("model id"),
                artifact_hash: ContentHash::parse("cd".repeat(32)).expect("model hash"),
                english_only: true,
            },
            options: TranscriptionOptions {
                language: Some("en".into()),
                initial_prompt: Some("custom   prompt".into()),
                ..TranscriptionOptions::default()
            },
            created_at: NOW,
        }
    }

    #[test]
    fn persists_corrected_transcription_and_replays_after_reopen() {
        let path = std::env::temp_dir().join(format!("speech-{}.sqlite3", RequestId::new()));
        let calls = Arc::new(Mutex::new(0));
        let request_id;
        let job_id;
        {
            let database = Database::open(&path).expect("database");
            let audio_asset_id = insert_audio(&database);
            let coordinator = SpeechTranscriptionCoordinator::new(&database, &database);
            let admitted = coordinator
                .admit(request(audio_asset_id))
                .expect("admitted");
            request_id = admitted.record.request.id;
            job_id = admitted.job.id;
            let work = coordinator
                .claim(
                    job_id,
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
                    &Audio,
                    &Library,
                    &Runtime {
                        calls: calls.clone(),
                        outcome: Ok(()),
                    },
                    CancellationReason::User,
                    TimestampMillis::new(2_000),
                )
                .expect("run");
            let SpeechTranscriptionRunResult::Succeeded(success) = outcome else {
                panic!("expected success");
            };
            let TranscriptionState::Succeeded { result } = success.record.state else {
                panic!("expected result");
            };
            assert_eq!(result.request_id, request_id);
            assert_eq!(result.raw_text, "lettuce a eye works");
            assert_eq!(result.corrected_text, "Lettuce AI works");
            assert!(!success.replayed);
        }
        let database = Database::open(&path).expect("reopen database");
        let replay = SpeechTranscriptionCoordinator::new(&database, &database)
            .replay(job_id)
            .expect("replay")
            .expect("completed result");
        assert!(replay.replayed);
        assert_eq!(replay.record.request.id, request_id);
        let repeated = SpeechTranscriptionCoordinator::new(&database, &database)
            .admit(replay.record.request.clone())
            .expect("repeat admission");
        assert!(!repeated.created);
        assert!(matches!(
            repeated.record.state,
            TranscriptionState::Succeeded { .. }
        ));
        assert_eq!(*calls.lock().expect("calls"), 1);
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn cancellation_and_transient_runtime_failure_settle_jobs() {
        let database = Database::open_in_memory().expect("database");
        let audio_asset_id = insert_audio(&database);
        let coordinator = SpeechTranscriptionCoordinator::new(&database, &database);
        let cancelled = coordinator
            .admit(request(audio_asset_id))
            .expect("cancel admission");
        let work = coordinator
            .claim(
                cancelled.job.id,
                WorkerId::new(),
                NOW,
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        work.handle.request_cancel();
        let outcome = coordinator
            .run(
                work,
                &Audio,
                &Library,
                &Runtime {
                    calls: Arc::new(Mutex::new(0)),
                    outcome: Ok(()),
                },
                CancellationReason::User,
                TimestampMillis::new(2_000),
            )
            .expect("cancelled run");
        assert!(
            matches!(outcome, SpeechTranscriptionRunResult::Cancelled { job, .. } if job.state == JobState::Cancelled)
        );

        let mut retry_request = request(audio_asset_id);
        retry_request.id = RequestId::new();
        let retry = coordinator.admit(retry_request).expect("retry admission");
        let work = coordinator
            .claim(
                retry.job.id,
                WorkerId::new(),
                TimestampMillis::new(3_000),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        let outcome = coordinator
            .run(
                work,
                &Audio,
                &Library,
                &Runtime {
                    calls: Arc::new(Mutex::new(0)),
                    outcome: Err(AsrRuntimeError::Unavailable),
                },
                CancellationReason::User,
                TimestampMillis::new(4_000),
            )
            .expect("retry run");
        assert!(
            matches!(outcome, SpeechTranscriptionRunResult::RetryScheduled { job, .. } if job.state == JobState::Queued)
        );
    }
}
