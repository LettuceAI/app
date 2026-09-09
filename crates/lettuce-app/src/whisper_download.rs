use std::{path::Path, time::Duration};

use async_trait::async_trait;
use lettuce_jobs::{
    BytesProgress, CancellationPolicy, CancellationReason, Claim, IdempotencyKey, JobError,
    JobErrorCode, JobKind, JobMutation, JobOutcome, JobPriority, JobSnapshot, JobState, JobStore,
    JobSubject, OutcomeRef, ProgressSnapshot, RecoveryPolicy, ResourceAvailability, ResourceClass,
    StageSnapshot, StoreError, SubjectKind, WorkerId, handle::JobHandle,
};
use lettuce_model_hub::{
    InstalledWhisperManifest, RemoteWhisperModel, WhisperDownloadSession,
    WhisperInstallPreparation, WhisperInstallStore, WhisperModelError, WhisperModelRepository,
    WhisperModelRepositoryError,
};
use lettuce_network::{ArtifactDownloadClient, ArtifactDownloadError, ArtifactDownloadStream};
use lettuce_types::{AssetId, JobId, TimestampMillis};
use uuid::Uuid;

const WHISPER_REPOSITORY: &str = "ggerganov/whisper.cpp";
const PROGRESS_INTERVAL_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WhisperDownloadSourceError {
    #[error("Whisper download transport failed")]
    Transport,
    #[error("Whisper download response is invalid")]
    InvalidResponse,
}

#[async_trait]
pub trait WhisperDownloadBody: Send {
    fn start(&self) -> u64;
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, WhisperDownloadSourceError>;
}

#[async_trait]
pub trait WhisperDownloadSource: Send + Sync {
    async fn open(
        &self,
        model: &RemoteWhisperModel,
        offset: u64,
    ) -> Result<Box<dyn WhisperDownloadBody>, WhisperDownloadSourceError>;
}

#[derive(Debug, Clone)]
pub struct HuggingFaceWhisperDownloadSource {
    client: ArtifactDownloadClient,
}

impl HuggingFaceWhisperDownloadSource {
    pub fn new() -> Result<Self, ArtifactDownloadError> {
        ArtifactDownloadClient::new().map(|client| Self { client })
    }
}

#[async_trait]
impl WhisperDownloadSource for HuggingFaceWhisperDownloadSource {
    async fn open(
        &self,
        model: &RemoteWhisperModel,
        offset: u64,
    ) -> Result<Box<dyn WhisperDownloadBody>, WhisperDownloadSourceError> {
        self.client
            .open_hugging_face(
                WHISPER_REPOSITORY,
                &model.source_revision,
                &model.filename,
                offset,
                model.byte_size,
            )
            .await
            .map(|stream| Box::new(stream) as Box<dyn WhisperDownloadBody>)
            .map_err(map_source_error)
    }
}

#[async_trait]
impl WhisperDownloadBody for ArtifactDownloadStream {
    fn start(&self) -> u64 {
        ArtifactDownloadStream::start(self)
    }

    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, WhisperDownloadSourceError> {
        ArtifactDownloadStream::next_chunk(self)
            .await
            .map_err(map_source_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperDownloadAdmission {
    pub model: RemoteWhisperModel,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug)]
pub struct WhisperDownloadClaimedWork {
    pub model: RemoteWhisperModel,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperDownloadSuccess {
    pub manifest: InstalledWhisperManifest,
    pub job: JobSnapshot,
    pub replayed: bool,
}

#[derive(Debug)]
pub enum WhisperDownloadRunResult {
    Succeeded(WhisperDownloadSuccess),
    Cancelled {
        error: WhisperDownloadError,
        job: JobSnapshot,
    },
    Failed {
        error: WhisperDownloadError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: WhisperDownloadError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum WhisperDownloadError {
    #[error("Whisper model is invalid: {0}")]
    Model(#[from] WhisperModelError),
    #[error("Whisper model persistence failed: {0}")]
    Repository(#[from] WhisperModelRepositoryError),
    #[error("Whisper download failed: {0}")]
    Source(#[from] WhisperDownloadSourceError),
    #[error("Whisper download job failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("Whisper download work is inconsistent")]
    InvalidWork,
    #[error("Whisper download was cancelled")]
    Cancelled,
}

#[derive(Debug)]
pub struct WhisperDownloadCoordinator<'a, R: ?Sized, J: ?Sized> {
    repository: &'a R,
    jobs: &'a J,
    installs: WhisperInstallStore,
}

impl<'a, R: ?Sized, J: ?Sized> WhisperDownloadCoordinator<'a, R, J> {
    pub fn new(
        repository: &'a R,
        jobs: &'a J,
        install_root: impl AsRef<Path>,
    ) -> Result<Self, WhisperDownloadError> {
        Ok(Self {
            repository,
            jobs,
            installs: WhisperInstallStore::open(install_root)?,
        })
    }
}

impl<R: WhisperModelRepository + ?Sized, J: JobStore + ?Sized>
    WhisperDownloadCoordinator<'_, R, J>
{
    pub fn admit(
        &self,
        model: RemoteWhisperModel,
    ) -> Result<WhisperDownloadAdmission, WhisperDownloadError> {
        model.validate()?;
        validate_existing(self.repository, &model)?;
        let asset_id = download_asset_id(&model);
        let subject = JobSubject::new(SubjectKind::ArtifactInstall, asset_id.to_string())
            .map_err(|_| WhisperDownloadError::InvalidWork)?;
        let key = IdempotencyKey::new(format!("whisper-install-{asset_id}"))
            .map_err(|_| WhisperDownloadError::InvalidWork)?;
        let admitted = self.jobs.create_or_get(
            lettuce_jobs::JobSpec::new(
                JobKind::ArtifactInstall,
                subject,
                OutcomeRef::ArtifactInstallation(asset_id),
            )
            .with_idempotency_key(key)
            .with_priority(JobPriority::Interactive)
            .with_resources(vec![
                ResourceClass::Network,
                ResourceClass::DiskRead,
                ResourceClass::DiskWrite,
                ResourceClass::Cpu,
            ])
            .with_policies(
                RecoveryPolicy::Resume,
                CancellationPolicy::UntilIrreversibleStage,
            ),
        )?;
        validate_job(&admitted.job, &model)?;
        Ok(WhisperDownloadAdmission {
            model,
            job: admitted.job,
            created: admitted.created,
        })
    }

    pub fn claim(
        &self,
        model: RemoteWhisperModel,
        job_id: JobId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<WhisperDownloadClaimedWork>, WhisperDownloadError> {
        model.validate()?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(WhisperDownloadError::InvalidWork)?;
        validate_job(&job, &model)?;
        if job.state == JobState::Succeeded {
            return Ok(None);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self.jobs.claim(job_id, worker_id, at, lease_for, allowed)? else {
            return Ok(None);
        };
        let handle = JobHandle::new(job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("download", false).expect("constant stage is valid"),
            at,
        })?;
        Ok(Some(WhisperDownloadClaimedWork {
            model,
            claim,
            handle,
            job,
        }))
    }

    pub fn replay(
        &self,
        model: &RemoteWhisperModel,
        job_id: JobId,
    ) -> Result<Option<WhisperDownloadSuccess>, WhisperDownloadError> {
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(WhisperDownloadError::InvalidWork)?;
        validate_job(&job, model)?;
        if job.state != JobState::Succeeded {
            return Ok(None);
        }
        let manifest =
            matching_manifest(self.repository, model)?.ok_or(WhisperDownloadError::InvalidWork)?;
        Ok(Some(WhisperDownloadSuccess {
            manifest,
            job,
            replayed: true,
        }))
    }

    pub async fn run<S: WhisperDownloadSource + ?Sized>(
        &self,
        work: WhisperDownloadClaimedWork,
        source: &S,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<WhisperDownloadRunResult, WhisperDownloadError> {
        validate_job(&work.job, &work.model)?;
        if work.job.state != JobState::Running
            || work.claim.claim.job_id != work.job.id
            || work.handle.id() != work.job.id
        {
            return Err(WhisperDownloadError::InvalidWork);
        }
        let outcome = self.execute(&work, source, now).await;
        match outcome {
            Ok((manifest, replayed)) => self.finish_success(work, manifest, replayed, now),
            Err(error)
                if matches!(error, WhisperDownloadError::Cancelled)
                    || work.handle.cancellation_token().is_cancelled() =>
            {
                self.finish_cancellation(work, error, cancellation_reason, now)
            }
            Err(error) => self.finish_error(work, error, now),
        }
    }

    async fn execute<S: WhisperDownloadSource + ?Sized>(
        &self,
        work: &WhisperDownloadClaimedWork,
        source: &S,
        now: TimestampMillis,
    ) -> Result<(InstalledWhisperManifest, bool), WhisperDownloadError> {
        check_cancelled(&work.handle)?;
        if let Some(manifest) = matching_manifest(self.repository, &work.model)? {
            manifest.verify()?;
            return Ok((manifest, true));
        }
        let preparation = self
            .installs
            .prepare(work.model.clone(), work.job.created_at)?;
        let manifest = match preparation {
            WhisperInstallPreparation::Installed(manifest) => manifest,
            WhisperInstallPreparation::Download(mut download) => {
                self.download(work, source, &mut download, now).await?;
                check_cancelled(&work.handle)?;
                self.jobs.append_and_transition(JobMutation::StageChanged {
                    claim: work.claim.claim.clone(),
                    stage: StageSnapshot::new("install", true).expect("constant stage is valid"),
                    at: now.max(work.job.updated_at),
                })?;
                download.finish()?
            }
        };
        let manifest = self.repository.admit_whisper_model(manifest)?;
        Ok((manifest, false))
    }

    async fn download<S: WhisperDownloadSource + ?Sized>(
        &self,
        work: &WhisperDownloadClaimedWork,
        source: &S,
        download: &mut WhisperDownloadSession,
        now: TimestampMillis,
    ) -> Result<(), WhisperDownloadError> {
        let mut body = source.open(&work.model, download.offset()).await?;
        if body.start() != download.offset() {
            if body.start() != 0 {
                return Err(WhisperDownloadSourceError::InvalidResponse.into());
            }
            download.restart()?;
        }
        let mut reported = download.offset();
        self.report_progress(work, reported, now)?;
        while let Some(chunk) = body.next_chunk().await? {
            check_cancelled(&work.handle)?;
            let completed = download.append(&chunk)?;
            if completed == work.model.byte_size
                || completed.saturating_sub(reported) >= PROGRESS_INTERVAL_BYTES
            {
                self.report_progress(work, completed, now)?;
                reported = completed;
            }
        }
        if download.offset() != work.model.byte_size {
            return Err(WhisperDownloadSourceError::Transport.into());
        }
        Ok(())
    }

    fn report_progress(
        &self,
        work: &WhisperDownloadClaimedWork,
        completed: u64,
        now: TimestampMillis,
    ) -> Result<(), WhisperDownloadError> {
        self.jobs.append_and_transition(JobMutation::Progress {
            claim: work.claim.claim.clone(),
            progress: ProgressSnapshot {
                bytes: Some(
                    BytesProgress::new(completed, Some(work.model.byte_size))
                        .map_err(|_| WhisperDownloadError::InvalidWork)?,
                ),
                ..ProgressSnapshot::default()
            },
            at: now.max(work.job.updated_at),
        })?;
        Ok(())
    }

    fn finish_success(
        &self,
        work: WhisperDownloadClaimedWork,
        manifest: InstalledWhisperManifest,
        replayed: bool,
        now: TimestampMillis,
    ) -> Result<WhisperDownloadRunResult, WhisperDownloadError> {
        let job = self.jobs.append_and_transition(JobMutation::Succeed {
            claim: work.claim.claim,
            outcome: JobOutcome::Success {
                result_ref: OutcomeRef::ArtifactInstallation(download_asset_id(&work.model)),
            },
            at: now.max(work.job.updated_at),
        })?;
        Ok(WhisperDownloadRunResult::Succeeded(
            WhisperDownloadSuccess {
                manifest,
                job,
                replayed,
            },
        ))
    }

    fn finish_cancellation(
        &self,
        work: WhisperDownloadClaimedWork,
        error: WhisperDownloadError,
        reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<WhisperDownloadRunResult, WhisperDownloadError> {
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
        Ok(WhisperDownloadRunResult::Cancelled { error, job })
    }

    fn finish_error(
        &self,
        work: WhisperDownloadClaimedWork,
        error: WhisperDownloadError,
        now: TimestampMillis,
    ) -> Result<WhisperDownloadRunResult, WhisperDownloadError> {
        let at = now.max(work.job.updated_at);
        let (code, retryable, label) = classify_error(&error);
        if retryable {
            let job = self
                .jobs
                .append_and_transition(JobMutation::RetryScheduled {
                    claim: work.claim.claim,
                    at,
                })?;
            return Ok(WhisperDownloadRunResult::RetryScheduled { error, job });
        }
        let job = self.jobs.append_and_transition(JobMutation::Fail {
            claim: work.claim.claim,
            error: JobError::new(code, false, label)
                .expect("constant download error label is valid"),
            at,
        })?;
        Ok(WhisperDownloadRunResult::Failed { error, job })
    }
}

fn check_cancelled(handle: &JobHandle) -> Result<(), WhisperDownloadError> {
    if handle.cancellation_token().is_cancelled() {
        Err(WhisperDownloadError::Cancelled)
    } else {
        Ok(())
    }
}

fn validate_existing<R: WhisperModelRepository + ?Sized>(
    repository: &R,
    model: &RemoteWhisperModel,
) -> Result<(), WhisperDownloadError> {
    if repository
        .get_whisper_model(&model.model_id)?
        .is_some_and(|manifest| {
            manifest.source_revision != model.source_revision
                || manifest.model.byte_size != model.byte_size
        })
    {
        return Err(WhisperModelRepositoryError::Conflict.into());
    }
    Ok(())
}

fn matching_manifest<R: WhisperModelRepository + ?Sized>(
    repository: &R,
    model: &RemoteWhisperModel,
) -> Result<Option<InstalledWhisperManifest>, WhisperDownloadError> {
    let manifest = repository.get_whisper_model(&model.model_id)?;
    match manifest {
        Some(manifest)
            if manifest.source_revision == model.source_revision
                && manifest.model.byte_size == model.byte_size =>
        {
            Ok(Some(manifest))
        }
        Some(_) => Err(WhisperModelRepositoryError::Conflict.into()),
        None => Ok(None),
    }
}

fn validate_job(job: &JobSnapshot, model: &RemoteWhisperModel) -> Result<(), WhisperDownloadError> {
    let asset_id = download_asset_id(model);
    if job.kind != JobKind::ArtifactInstall
        || job.subject.kind != SubjectKind::ArtifactInstall
        || job.subject.id.as_str() != asset_id.to_string()
        || (job.state == JobState::Succeeded
            && job.outcome.as_ref()
                != Some(&JobOutcome::Success {
                    result_ref: OutcomeRef::ArtifactInstallation(asset_id),
                }))
    {
        return Err(WhisperDownloadError::InvalidWork);
    }
    Ok(())
}

fn download_asset_id(model: &RemoteWhisperModel) -> AssetId {
    let identity = format!(
        "whisper\0{}\0{}\0{}\0{}",
        model.model_id, model.source_revision, model.byte_size, model.sha256
    );
    AssetId::from_uuid(Uuid::new_v5(&Uuid::NAMESPACE_URL, identity.as_bytes()))
}

fn map_source_error(error: ArtifactDownloadError) -> WhisperDownloadSourceError {
    match error {
        ArtifactDownloadError::Transport => WhisperDownloadSourceError::Transport,
        ArtifactDownloadError::InvalidRequest | ArtifactDownloadError::InvalidResponse => {
            WhisperDownloadSourceError::InvalidResponse
        }
    }
}

fn classify_error(error: &WhisperDownloadError) -> (JobErrorCode, bool, &'static str) {
    match error {
        WhisperDownloadError::Source(WhisperDownloadSourceError::Transport) => (
            JobErrorCode::ResourceUnavailable,
            true,
            "download unavailable",
        ),
        WhisperDownloadError::Model(WhisperModelError::Unreadable)
        | WhisperDownloadError::Repository(WhisperModelRepositoryError::Storage) => (
            JobErrorCode::StorageFailure,
            true,
            "download storage unavailable",
        ),
        WhisperDownloadError::Model(WhisperModelError::Mismatch)
        | WhisperDownloadError::Source(WhisperDownloadSourceError::InvalidResponse) => (
            JobErrorCode::IntegrityFailure,
            false,
            "download integrity failed",
        ),
        WhisperDownloadError::Cancelled => (JobErrorCode::Cancelled, false, "download cancelled"),
        WhisperDownloadError::Jobs(StoreError::StaleLease | StoreError::LeaseExpired) => {
            (JobErrorCode::LeaseLost, true, "download lease lost")
        }
        _ => (
            JobErrorCode::InvalidInput,
            false,
            "download input is invalid",
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use super::*;
    use lettuce_database::Database;
    use lettuce_jobs::{JobStore, events::JobEvent};

    #[derive(Debug)]
    struct FixtureSource {
        bytes: Vec<u8>,
        fail_after_first_chunk: bool,
        offsets: Mutex<Vec<u64>>,
    }

    #[derive(Debug)]
    struct FixtureBody {
        start: u64,
        chunks: VecDeque<Result<Vec<u8>, WhisperDownloadSourceError>>,
    }

    #[async_trait]
    impl WhisperDownloadSource for FixtureSource {
        async fn open(
            &self,
            _model: &RemoteWhisperModel,
            offset: u64,
        ) -> Result<Box<dyn WhisperDownloadBody>, WhisperDownloadSourceError> {
            self.offsets.lock().expect("offsets").push(offset);
            let remaining = self.bytes[offset as usize..].to_vec();
            let chunks = if self.fail_after_first_chunk {
                let split = remaining.len().min(5);
                VecDeque::from([
                    Ok(remaining[..split].to_vec()),
                    Err(WhisperDownloadSourceError::Transport),
                ])
            } else {
                VecDeque::from([Ok(remaining)])
            };
            Ok(Box::new(FixtureBody {
                start: offset,
                chunks,
            }))
        }
    }

    #[async_trait]
    impl WhisperDownloadBody for FixtureBody {
        fn start(&self) -> u64 {
            self.start
        }

        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, WhisperDownloadSourceError> {
            self.chunks.pop_front().transpose()
        }
    }

    #[tokio::test]
    async fn resumes_partial_download_installs_once_and_replays_without_network() {
        let root = std::env::temp_dir().join(format!("whisper-job-{}", JobId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let database_path = root.join("jobs.sqlite3");
        let install_root = root.join("models");
        let bytes = b"downloaded model bytes".to_vec();
        let model = RemoteWhisperModel::pinned(
            "ggml-base.bin",
            "ab".repeat(20),
            bytes.len() as u64,
            "ed68e1b9289be4deda8384e06007e492ff725196fba29d1c78ea3280bfa9690c",
        )
        .expect("remote model");
        let job_id;
        {
            let database = Database::open(&database_path).expect("database");
            let coordinator = WhisperDownloadCoordinator::new(&database, &database, &install_root)
                .expect("coordinator");
            let admitted = coordinator.admit(model.clone()).expect("admission");
            job_id = admitted.job.id;
            let work = coordinator
                .claim(
                    model.clone(),
                    job_id,
                    WorkerId::new(),
                    TimestampMillis::new(10),
                    Duration::from_secs(60),
                    &ResourceAvailability::all(),
                )
                .expect("claim")
                .expect("work");
            let source = FixtureSource {
                bytes: bytes.clone(),
                fail_after_first_chunk: true,
                offsets: Mutex::new(Vec::new()),
            };
            let result = coordinator
                .run(
                    work,
                    &source,
                    CancellationReason::User,
                    TimestampMillis::new(11),
                )
                .await
                .expect("run");
            assert!(matches!(
                result,
                WhisperDownloadRunResult::RetryScheduled { .. }
            ));
        }

        let database = Database::open(&database_path).expect("reopen database");
        let coordinator = WhisperDownloadCoordinator::new(&database, &database, &install_root)
            .expect("reopened coordinator");
        assert!(
            !coordinator
                .admit(model.clone())
                .expect("replay admission")
                .created
        );
        let work = coordinator
            .claim(
                model.clone(),
                job_id,
                WorkerId::new(),
                TimestampMillis::new(12),
                Duration::from_secs(60),
                &ResourceAvailability::all(),
            )
            .expect("reclaim")
            .expect("reclaimed work");
        let source = FixtureSource {
            bytes: bytes.clone(),
            fail_after_first_chunk: false,
            offsets: Mutex::new(Vec::new()),
        };
        let result = coordinator
            .run(
                work,
                &source,
                CancellationReason::User,
                TimestampMillis::new(13),
            )
            .await
            .expect("resumed run");
        let WhisperDownloadRunResult::Succeeded(success) = result else {
            panic!("expected success");
        };
        assert!(!success.replayed);
        assert_eq!(*source.offsets.lock().expect("offsets"), vec![5]);
        assert_eq!(
            std::fs::read(install_root.join("base/ggml-base.bin")).expect("installed bytes"),
            bytes
        );
        success.manifest.verify().expect("verified manifest");
        assert!(
            coordinator
                .replay(&model, job_id)
                .expect("replay")
                .is_some_and(|value| value.replayed)
        );
        let events = JobStore::events_since(&database, job_id, None, 100).expect("events");
        assert!(events.iter().any(|event| {
            matches!(
                &event.event,
                JobEvent::Progressed { progress }
                    if progress.bytes.as_ref().is_some_and(|value| value.completed == 5)
            )
        }));
        drop(coordinator);
        drop(database);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[tokio::test]
    async fn cancellation_before_transfer_keeps_job_and_install_consistent() {
        let root = std::env::temp_dir().join(format!("whisper-cancel-{}", JobId::new()));
        let database = Database::open_in_memory().expect("database");
        let bytes = b"model".to_vec();
        let model = RemoteWhisperModel::pinned(
            "ggml-tiny.bin",
            "ab".repeat(20),
            bytes.len() as u64,
            "9372c470eeadd5ecd9c3c74c2b3cb633f8e2f2fad799250a0f70d652b6b825e4",
        )
        .expect("remote model");
        let coordinator =
            WhisperDownloadCoordinator::new(&database, &database, &root).expect("coordinator");
        let admitted = coordinator.admit(model.clone()).expect("admission");
        let work = coordinator
            .claim(
                model,
                admitted.job.id,
                WorkerId::new(),
                TimestampMillis::new(10),
                Duration::from_secs(60),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        work.handle.request_cancel();
        let source = FixtureSource {
            bytes,
            fail_after_first_chunk: false,
            offsets: Mutex::new(Vec::new()),
        };
        let result = coordinator
            .run(
                work,
                &source,
                CancellationReason::User,
                TimestampMillis::new(11),
            )
            .await
            .expect("cancelled run");
        assert!(matches!(
            result,
            WhisperDownloadRunResult::Cancelled { job, .. } if job.state == JobState::Cancelled
        ));
        assert!(source.offsets.lock().expect("offsets").is_empty());
        assert!(!root.join("tiny/ggml-tiny.bin").exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
