use std::time::Duration;

use async_trait::async_trait;
use lettuce_jobs::{
    BytesProgress, CancellationPolicy, CancellationReason, Claim, IdempotencyKey, JobError,
    JobErrorCode, JobKind, JobMutation, JobOutcome, JobPriority, JobSnapshot, JobState, JobStore,
    JobSubject, OutcomeRef, ProgressSnapshot, RecoveryPolicy, ResourceAvailability, ResourceClass,
    StageSnapshot, StoreError, SubjectKind, WorkerId, handle::JobHandle,
};
use lettuce_model_hub::{
    InstalledKokoroModel, KOKORO_REPOSITORY, KokoroArtifactPreparation, KokoroDownloadSession,
    KokoroInstallError, KokoroInstallStore, RemoteKokoroArtifact, RemoteKokoroModel,
};
use lettuce_network::{ArtifactDownloadClient, ArtifactDownloadError, ArtifactDownloadStream};
use lettuce_types::{AssetId, JobId, TimestampMillis};
use uuid::Uuid;

const PROGRESS_INTERVAL_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KokoroDownloadSourceError {
    #[error("Kokoro download transport failed")]
    Transport,
    #[error("Kokoro download response is invalid")]
    InvalidResponse,
}

#[async_trait]
pub trait KokoroDownloadBody: Send {
    fn start(&self) -> u64;
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, KokoroDownloadSourceError>;
}

#[async_trait]
pub trait KokoroDownloadSource: Send + Sync {
    async fn open(
        &self,
        model: &RemoteKokoroModel,
        artifact: &RemoteKokoroArtifact,
        offset: u64,
    ) -> Result<Box<dyn KokoroDownloadBody>, KokoroDownloadSourceError>;
}

#[derive(Debug, Clone)]
pub struct HuggingFaceKokoroDownloadSource {
    client: ArtifactDownloadClient,
}

impl HuggingFaceKokoroDownloadSource {
    pub fn new() -> Result<Self, ArtifactDownloadError> {
        ArtifactDownloadClient::new().map(|client| Self { client })
    }
}

#[async_trait]
impl KokoroDownloadSource for HuggingFaceKokoroDownloadSource {
    async fn open(
        &self,
        model: &RemoteKokoroModel,
        artifact: &RemoteKokoroArtifact,
        offset: u64,
    ) -> Result<Box<dyn KokoroDownloadBody>, KokoroDownloadSourceError> {
        self.client
            .open_hugging_face(
                KOKORO_REPOSITORY,
                model.source_revision,
                artifact.remote_path,
                offset,
                artifact.byte_size,
            )
            .await
            .map(|stream| Box::new(stream) as Box<dyn KokoroDownloadBody>)
            .map_err(map_source_error)
    }
}

#[async_trait]
impl KokoroDownloadBody for ArtifactDownloadStream {
    fn start(&self) -> u64 {
        ArtifactDownloadStream::start(self)
    }

    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, KokoroDownloadSourceError> {
        ArtifactDownloadStream::next_chunk(self)
            .await
            .map_err(map_source_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroDownloadAdmission {
    pub model: RemoteKokoroModel,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug)]
pub struct KokoroDownloadClaimedWork {
    pub model: RemoteKokoroModel,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroDownloadSuccess {
    pub model: InstalledKokoroModel,
    pub job: JobSnapshot,
    pub replayed: bool,
}

#[derive(Debug)]
pub enum KokoroDownloadRunResult {
    Succeeded(KokoroDownloadSuccess),
    Cancelled {
        error: KokoroDownloadError,
        job: JobSnapshot,
    },
    Failed {
        error: KokoroDownloadError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: KokoroDownloadError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum KokoroDownloadError {
    #[error("Kokoro model installation failed: {0}")]
    Install(#[from] KokoroInstallError),
    #[error("Kokoro model download failed: {0}")]
    Source(#[from] KokoroDownloadSourceError),
    #[error("Kokoro model job failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("Kokoro model download work is inconsistent")]
    InvalidWork,
    #[error("Kokoro model download was cancelled")]
    Cancelled,
}

#[derive(Debug)]
pub struct KokoroDownloadCoordinator<'a, J: ?Sized> {
    jobs: &'a J,
    installs: KokoroInstallStore,
}

#[derive(Debug, Clone, Copy)]
struct KokoroDownloadProgress {
    completed_before: u64,
    at: TimestampMillis,
    cancellable: bool,
}

impl<'a, J: ?Sized> KokoroDownloadCoordinator<'a, J> {
    #[must_use]
    pub const fn new(jobs: &'a J, installs: KokoroInstallStore) -> Self {
        Self { jobs, installs }
    }
}

impl<J: JobStore + ?Sized> KokoroDownloadCoordinator<'_, J> {
    pub fn admit(
        &self,
        model: RemoteKokoroModel,
    ) -> Result<KokoroDownloadAdmission, KokoroDownloadError> {
        model.validate()?;
        if !lettuce_model_hub::kokoro_platform_allows_variant(model.variant) {
            return Err(KokoroDownloadError::InvalidWork);
        }
        let asset_id = download_asset_id(&model);
        let subject = JobSubject::new(SubjectKind::ArtifactInstall, asset_id.to_string())
            .map_err(|_| KokoroDownloadError::InvalidWork)?;
        let mut key = format!("kokoro-install-{asset_id}");
        for _ in 0..64 {
            let admitted = self.jobs.create_or_get(
                lettuce_jobs::JobSpec::new(
                    JobKind::ArtifactInstall,
                    subject.clone(),
                    OutcomeRef::ArtifactInstallation(asset_id),
                )
                .with_idempotency_key(
                    IdempotencyKey::new(key).map_err(|_| KokoroDownloadError::InvalidWork)?,
                )
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
            if admitted.job.state != JobState::Succeeded
                || self.installs.installed(&model)?.is_some()
            {
                return Ok(KokoroDownloadAdmission {
                    model,
                    job: admitted.job,
                    created: admitted.created,
                });
            }
            key = format!("kokoro-install-{asset_id}-after-{}", admitted.job.id);
        }
        Err(KokoroDownloadError::InvalidWork)
    }

    pub fn claim(
        &self,
        model: RemoteKokoroModel,
        job_id: JobId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<KokoroDownloadClaimedWork>, KokoroDownloadError> {
        model.validate()?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(KokoroDownloadError::InvalidWork)?;
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
            stage: StageSnapshot::new("download", false).expect("constant stage"),
            at,
        })?;
        Ok(Some(KokoroDownloadClaimedWork {
            model,
            claim,
            handle,
            job,
        }))
    }

    pub fn replay(
        &self,
        model: &RemoteKokoroModel,
        job_id: JobId,
    ) -> Result<Option<KokoroDownloadSuccess>, KokoroDownloadError> {
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(KokoroDownloadError::InvalidWork)?;
        validate_job(&job, model)?;
        if job.state != JobState::Succeeded {
            return Ok(None);
        }
        let installed = self
            .installs
            .installed(model)?
            .ok_or(KokoroDownloadError::InvalidWork)?;
        Ok(Some(KokoroDownloadSuccess {
            model: installed,
            job,
            replayed: true,
        }))
    }

    pub async fn run<S: KokoroDownloadSource + ?Sized>(
        &self,
        work: KokoroDownloadClaimedWork,
        source: &S,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<KokoroDownloadRunResult, KokoroDownloadError> {
        validate_job(&work.job, &work.model)?;
        if work.job.state != JobState::Running
            || work.claim.claim.job_id != work.job.id
            || work.handle.id() != work.job.id
        {
            return Err(KokoroDownloadError::InvalidWork);
        }
        let result = self.execute(&work, source, now).await;
        match result {
            Ok((model, replayed)) => self.finish_success(work, model, replayed, now),
            Err(error) if matches!(error, KokoroDownloadError::Cancelled) => {
                self.finish_cancellation(work, error, cancellation_reason, now)
            }
            Err(error) => self.finish_error(work, error, now),
        }
    }

    async fn execute<S: KokoroDownloadSource + ?Sized>(
        &self,
        work: &KokoroDownloadClaimedWork,
        source: &S,
        now: TimestampMillis,
    ) -> Result<(InstalledKokoroModel, bool), KokoroDownloadError> {
        check_cancelled(&work.handle)?;
        if let Some(installed) = self.installs.installed(&work.model)? {
            return Ok((installed, true));
        }
        let mut completed = 0_u64;
        let mut irreversible = false;
        for artifact in work.model.artifacts.clone() {
            if !irreversible {
                check_cancelled(&work.handle)?;
            }
            match self
                .installs
                .prepare(work.model.source_revision, artifact.clone())?
            {
                KokoroArtifactPreparation::Installed(_) => {
                    completed += artifact.byte_size;
                    self.report_progress(work, completed, now)?;
                }
                KokoroArtifactPreparation::Download(mut download) => {
                    self.download(
                        work,
                        source,
                        &artifact,
                        &mut download,
                        KokoroDownloadProgress {
                            completed_before: completed,
                            at: now,
                            cancellable: !irreversible,
                        },
                    )
                    .await?;
                    if !irreversible {
                        check_cancelled(&work.handle)?;
                        self.jobs.append_and_transition(JobMutation::StageChanged {
                            claim: work.claim.claim.clone(),
                            stage: StageSnapshot::new("install", true).expect("constant stage"),
                            at: now.max(work.job.updated_at),
                        })?;
                        irreversible = true;
                    }
                    download.finish()?;
                    completed += artifact.byte_size;
                }
            }
        }
        let installed = self
            .installs
            .installed(&work.model)?
            .ok_or(KokoroDownloadError::InvalidWork)?;
        Ok((installed, false))
    }

    async fn download<S: KokoroDownloadSource + ?Sized>(
        &self,
        work: &KokoroDownloadClaimedWork,
        source: &S,
        artifact: &RemoteKokoroArtifact,
        session: &mut KokoroDownloadSession,
        progress: KokoroDownloadProgress,
    ) -> Result<(), KokoroDownloadError> {
        if progress.cancellable {
            check_cancelled(&work.handle)?;
        }
        let mut body = source.open(&work.model, artifact, session.offset()).await?;
        if body.start() != session.offset() {
            if body.start() != 0 {
                return Err(KokoroDownloadSourceError::InvalidResponse.into());
            }
            session.restart()?;
        }
        let mut reported = session.offset();
        self.report_progress(work, progress.completed_before + reported, progress.at)?;
        while let Some(chunk) = body.next_chunk().await? {
            if progress.cancellable {
                check_cancelled(&work.handle)?;
            }
            let offset = session.append(&chunk)?;
            if offset == artifact.byte_size
                || offset.saturating_sub(reported) >= PROGRESS_INTERVAL_BYTES
            {
                self.report_progress(work, progress.completed_before + offset, progress.at)?;
                reported = offset;
            }
        }
        if session.offset() != artifact.byte_size {
            return Err(KokoroDownloadSourceError::Transport.into());
        }
        Ok(())
    }

    fn report_progress(
        &self,
        work: &KokoroDownloadClaimedWork,
        completed: u64,
        now: TimestampMillis,
    ) -> Result<(), KokoroDownloadError> {
        self.jobs.append_and_transition(JobMutation::Progress {
            claim: work.claim.claim.clone(),
            progress: ProgressSnapshot {
                bytes: Some(
                    BytesProgress::new(completed, Some(work.model.total_bytes()))
                        .map_err(|_| KokoroDownloadError::InvalidWork)?,
                ),
                ..ProgressSnapshot::default()
            },
            at: now.max(work.job.updated_at),
        })?;
        Ok(())
    }

    fn finish_success(
        &self,
        work: KokoroDownloadClaimedWork,
        model: InstalledKokoroModel,
        replayed: bool,
        now: TimestampMillis,
    ) -> Result<KokoroDownloadRunResult, KokoroDownloadError> {
        let job = self.jobs.append_and_transition(JobMutation::Succeed {
            claim: work.claim.claim,
            outcome: JobOutcome::Success {
                result_ref: OutcomeRef::ArtifactInstallation(download_asset_id(&work.model)),
            },
            at: now.max(work.job.updated_at),
        })?;
        Ok(KokoroDownloadRunResult::Succeeded(KokoroDownloadSuccess {
            model,
            job,
            replayed,
        }))
    }

    fn finish_cancellation(
        &self,
        work: KokoroDownloadClaimedWork,
        error: KokoroDownloadError,
        reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<KokoroDownloadRunResult, KokoroDownloadError> {
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
        Ok(KokoroDownloadRunResult::Cancelled { error, job })
    }

    fn finish_error(
        &self,
        work: KokoroDownloadClaimedWork,
        error: KokoroDownloadError,
        now: TimestampMillis,
    ) -> Result<KokoroDownloadRunResult, KokoroDownloadError> {
        let at = now.max(work.job.updated_at);
        let (code, retryable, label) = classify_error(&error);
        if retryable {
            let job = self
                .jobs
                .append_and_transition(JobMutation::RetryScheduled {
                    claim: work.claim.claim,
                    at,
                })?;
            return Ok(KokoroDownloadRunResult::RetryScheduled { error, job });
        }
        let job = self.jobs.append_and_transition(JobMutation::Fail {
            claim: work.claim.claim,
            error: JobError::new(code, false, label).expect("constant error label"),
            at,
        })?;
        Ok(KokoroDownloadRunResult::Failed { error, job })
    }
}

fn check_cancelled(handle: &JobHandle) -> Result<(), KokoroDownloadError> {
    if handle.cancellation_token().is_cancelled() {
        Err(KokoroDownloadError::Cancelled)
    } else {
        Ok(())
    }
}

fn validate_job(job: &JobSnapshot, model: &RemoteKokoroModel) -> Result<(), KokoroDownloadError> {
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
        return Err(KokoroDownloadError::InvalidWork);
    }
    Ok(())
}

fn download_asset_id(model: &RemoteKokoroModel) -> AssetId {
    let mut identity = format!("kokoro\0{}\0{}", model.variant.id(), model.source_revision);
    for artifact in &model.artifacts {
        identity.push_str(&format!(
            "\0{}\0{}\0{}",
            artifact.remote_path, artifact.byte_size, artifact.sha256
        ));
    }
    AssetId::from_uuid(Uuid::new_v5(&Uuid::NAMESPACE_URL, identity.as_bytes()))
}

fn classify_error(error: &KokoroDownloadError) -> (JobErrorCode, bool, &'static str) {
    match error {
        KokoroDownloadError::Source(KokoroDownloadSourceError::Transport) => (
            JobErrorCode::ResourceUnavailable,
            true,
            "download unavailable",
        ),
        KokoroDownloadError::Install(KokoroInstallError::Platform(
            lettuce_platform::PlatformError::Io
            | lettuce_platform::PlatformError::RecoveryNeeded
            | lettuce_platform::PlatformError::ReplaceFailed,
        )) => (
            JobErrorCode::StorageFailure,
            true,
            "download storage unavailable",
        ),
        KokoroDownloadError::Install(KokoroInstallError::Unreadable) => (
            JobErrorCode::StorageFailure,
            true,
            "download storage unavailable",
        ),
        KokoroDownloadError::Install(
            KokoroInstallError::Mismatch | KokoroInstallError::InvalidArtifact,
        )
        | KokoroDownloadError::Source(KokoroDownloadSourceError::InvalidResponse) => (
            JobErrorCode::IntegrityFailure,
            false,
            "download integrity failed",
        ),
        KokoroDownloadError::Cancelled => (JobErrorCode::Cancelled, false, "download cancelled"),
        KokoroDownloadError::Jobs(StoreError::StaleLease | StoreError::LeaseExpired) => {
            (JobErrorCode::LeaseLost, true, "download lease lost")
        }
        _ => (
            JobErrorCode::InvalidInput,
            false,
            "download input is invalid",
        ),
    }
}

fn map_source_error(error: ArtifactDownloadError) -> KokoroDownloadSourceError {
    match error {
        ArtifactDownloadError::Transport => KokoroDownloadSourceError::Transport,
        ArtifactDownloadError::InvalidRequest | ArtifactDownloadError::InvalidResponse => {
            KokoroDownloadSourceError::InvalidResponse
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use lettuce_database::Database;
    use lettuce_model_hub::{KokoroArtifactRole, KokoroModelVariant};

    use super::*;

    const REVISION: &str = "abababababababababababababababababababab";

    #[derive(Debug)]
    struct FixtureSource {
        fail_once: Mutex<bool>,
        offsets: Mutex<Vec<(String, u64)>>,
    }

    #[derive(Debug)]
    struct FixtureBody {
        start: u64,
        chunks: VecDeque<Result<Vec<u8>, KokoroDownloadSourceError>>,
    }

    #[async_trait]
    impl KokoroDownloadSource for FixtureSource {
        async fn open(
            &self,
            _model: &RemoteKokoroModel,
            artifact: &RemoteKokoroArtifact,
            offset: u64,
        ) -> Result<Box<dyn KokoroDownloadBody>, KokoroDownloadSourceError> {
            self.offsets
                .lock()
                .expect("offsets")
                .push((artifact.remote_path.to_owned(), offset));
            let bytes = bytes_for(artifact.remote_path);
            let remaining = bytes[offset as usize..].to_vec();
            let chunks = if *self.fail_once.lock().expect("failure flag") {
                *self.fail_once.lock().expect("failure flag") = false;
                let split = remaining.len().min(2);
                VecDeque::from([
                    Ok(remaining[..split].to_vec()),
                    Err(KokoroDownloadSourceError::Transport),
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
    impl KokoroDownloadBody for FixtureBody {
        fn start(&self) -> u64 {
            self.start
        }

        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, KokoroDownloadSourceError> {
            self.chunks.pop_front().transpose()
        }
    }

    fn model() -> RemoteKokoroModel {
        RemoteKokoroModel {
            variant: KokoroModelVariant::Int8,
            source_revision: REVISION,
            artifacts: vec![
                artifact(
                    KokoroArtifactRole::Config,
                    "config.json",
                    &["config.json"],
                    6,
                    "b79606fb3afea5bd1609ed40b622142f1c98125abcfe89a76a661b0e8e343910",
                ),
                artifact(
                    KokoroArtifactRole::Tokenizer,
                    "tokenizer.json",
                    &["tokenizer.json"],
                    9,
                    "5f97e3774c51edd1d63706c2ec3826c564a067794770cdab0f8c4797971cacf9",
                ),
                artifact(
                    KokoroArtifactRole::TokenizerConfig,
                    "tokenizer_config.json",
                    &["tokenizer_config.json"],
                    16,
                    "f79efbec9e39dcb41bc88948deff80bee846fe148f24197baac1835ccb83bc2a",
                ),
                artifact(
                    KokoroArtifactRole::Model,
                    "onnx/model_quantized.onnx",
                    &["onnx", "model_quantized.onnx"],
                    5,
                    "9372c470eeadd5ecd9c3c74c2b3cb633f8e2f2fad799250a0f70d652b6b825e4",
                ),
            ],
        }
    }

    fn artifact(
        role: KokoroArtifactRole,
        remote_path: &'static str,
        local_segments: &'static [&'static str],
        byte_size: u64,
        sha256: &'static str,
    ) -> RemoteKokoroArtifact {
        RemoteKokoroArtifact {
            role,
            remote_path,
            local_segments,
            byte_size,
            sha256,
        }
    }

    fn bytes_for(path: &str) -> &'static [u8] {
        match path {
            "config.json" => b"config",
            "tokenizer.json" => b"tokenizer",
            "tokenizer_config.json" => b"tokenizer-config",
            "onnx/model_quantized.onnx" => b"model",
            _ => panic!("unexpected fixture path"),
        }
    }

    #[tokio::test]
    async fn resumes_four_file_install_and_replays_after_reopen() {
        let root = std::env::temp_dir().join(format!("kokoro-job-{}", JobId::new()));
        let database_path = root.join("jobs.sqlite3");
        let install_root = root.join("assets");
        std::fs::create_dir_all(&root).expect("root");
        let source = FixtureSource {
            fail_once: Mutex::new(true),
            offsets: Mutex::new(Vec::new()),
        };
        let model = model();
        let job_id;
        {
            let database = Database::open(&database_path).expect("database");
            let installs = KokoroInstallStore::open(&install_root).expect("install store");
            let coordinator = KokoroDownloadCoordinator::new(&database, installs);
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
            assert!(matches!(
                coordinator
                    .run(
                        work,
                        &source,
                        CancellationReason::User,
                        TimestampMillis::new(11)
                    )
                    .await
                    .expect("run"),
                KokoroDownloadRunResult::RetryScheduled { .. }
            ));
        }
        let database = Database::open(&database_path).expect("reopen database");
        let installs = KokoroInstallStore::open(&install_root).expect("install store");
        let coordinator = KokoroDownloadCoordinator::new(&database, installs);
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
            .expect("work");
        let result = coordinator
            .run(
                work,
                &source,
                CancellationReason::User,
                TimestampMillis::new(13),
            )
            .await
            .expect("run");
        let KokoroDownloadRunResult::Succeeded(success) = result else {
            panic!("expected success");
        };
        assert_eq!(success.model.artifacts.len(), 4);
        assert!(!success.replayed);
        assert!(
            coordinator
                .replay(&model, job_id)
                .expect("replay")
                .is_some_and(|result| result.replayed)
        );
        assert_eq!(
            source.offsets.lock().expect("offsets")[1],
            ("config.json".to_owned(), 2)
        );
        assert_eq!(
            std::fs::read(install_root.join("onnx/model_quantized.onnx")).expect("model"),
            b"model"
        );
        let removal_store = KokoroInstallStore::open(&install_root).expect("removal store");
        assert!(
            crate::remove_managed_kokoro_model(&removal_store, &model)
                .expect("remove model")
                .removed
        );
        assert!(install_root.join("config.json").exists());
        assert!(!install_root.join("onnx/model_quantized.onnx").exists());
        let replacement = coordinator
            .admit(model.clone())
            .expect("replacement admission");
        assert!(replacement.created);
        assert_ne!(replacement.job.id, job_id);
        drop(coordinator);
        drop(database);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[tokio::test]
    async fn cancellation_before_transfer_settles_without_installed_files() {
        let root = std::env::temp_dir().join(format!("kokoro-cancel-{}", JobId::new()));
        let database = Database::open_in_memory().expect("database");
        let installs = KokoroInstallStore::open(&root).expect("install store");
        let coordinator = KokoroDownloadCoordinator::new(&database, installs);
        let model = model();
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
            fail_once: Mutex::new(false),
            offsets: Mutex::new(Vec::new()),
        };
        assert!(matches!(
            coordinator
                .run(
                    work,
                    &source,
                    CancellationReason::User,
                    TimestampMillis::new(11)
                )
                .await
                .expect("run"),
            KokoroDownloadRunResult::Cancelled { .. }
        ));
        assert!(source.offsets.lock().expect("offsets").is_empty());
        assert!(!root.join("config.json").exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
