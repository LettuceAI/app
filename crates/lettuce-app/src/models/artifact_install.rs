//! Installs a set of pinned files (Hugging Face or HTTPS) as one
//! `ArtifactInstall` job: resumable, verified downloads below one root.
//! Engine builds, catalog image models and the upscaler use it.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use lettuce_jobs::JobQuery;
use lettuce_jobs::{
    BytesProgress, CancellationPolicy, CancellationReason, Claim, IdempotencyKey, JobError,
    JobErrorCode, JobKind, JobMutation, JobOutcome, JobPriority, JobSnapshot, JobSpec, JobState,
    JobStore, JobSubject, OutcomeRef, ProgressSnapshot, RecoveryPolicy, ResourceAvailability,
    ResourceClass, StageSnapshot, StoreError, SubjectKind, WorkerId,
    handle::{CancellationToken, JobHandle},
};
use lettuce_model_hub::{
    PinnedArtifact, PinnedArtifactError, PinnedArtifactPreparation, PinnedArtifactStore,
    PinnedDownload,
};
use lettuce_network::{ArtifactDownloadClient, ArtifactDownloadError, ArtifactDownloadStream};
use lettuce_types::{AssetId, JobId, PageRequest, TimestampMillis};
use uuid::Uuid;

const PROGRESS_INTERVAL_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactSource {
    HuggingFace {
        repository: String,
        revision: String,
        path: String,
    },
    Https {
        url: String,
    },
}

impl ArtifactSource {
    #[must_use]
    pub fn identity(&self) -> String {
        match self {
            Self::HuggingFace {
                repository,
                revision,
                path,
            } => format!("hf:{repository}@{revision}/{path}"),
            Self::Https { url } => url.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedArtifact {
    pub source: ArtifactSource,
    pub artifact: PinnedArtifact,
}

/// Everything one install downloads, below one root folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactInstallPlan {
    /// A stable description of what is installed (install ids such as
    /// `sdcpp:<profile>:<variant>:<release>:<asset>`).
    pub install_id: String,
    pub root: PathBuf,
    pub artifacts: Vec<PlannedArtifact>,
}

impl ArtifactInstallPlan {
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.artifacts
            .iter()
            .map(|planned| planned.artifact.byte_size)
            .sum()
    }

    fn outcome_asset_id(&self) -> AssetId {
        let mut identity = format!("artifact-install\0{}", self.install_id);
        for planned in &self.artifacts {
            identity.push('\0');
            identity.push_str(&planned.artifact.source_identity);
            identity.push('\0');
            identity.push_str(&planned.artifact.local_segments.join("/"));
        }
        let uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes());
        uuid.to_string()
            .parse()
            .expect("a UUID is a valid asset id")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArtifactSourceError {
    #[error("artifact download transport failed")]
    Transport,
    #[error("artifact download response is invalid")]
    InvalidResponse,
    #[error("artifact download request is invalid")]
    InvalidRequest,
}

#[async_trait]
pub trait ArtifactBody: Send {
    fn start(&self) -> u64;
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ArtifactSourceError>;
}

#[async_trait]
pub trait ArtifactSourceClient: Send + Sync {
    async fn open(
        &self,
        source: &ArtifactSource,
        offset: u64,
        expected_size: u64,
    ) -> Result<Box<dyn ArtifactBody>, ArtifactSourceError>;
}

fn map_download_error(error: ArtifactDownloadError) -> ArtifactSourceError {
    match error {
        ArtifactDownloadError::InvalidRequest => ArtifactSourceError::InvalidRequest,
        ArtifactDownloadError::InvalidResponse => ArtifactSourceError::InvalidResponse,
        ArtifactDownloadError::Transport => ArtifactSourceError::Transport,
    }
}

#[async_trait]
impl ArtifactBody for ArtifactDownloadStream {
    fn start(&self) -> u64 {
        ArtifactDownloadStream::start(self)
    }

    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ArtifactSourceError> {
        ArtifactDownloadStream::next_chunk(self)
            .await
            .map_err(map_download_error)
    }
}

#[async_trait]
impl ArtifactSourceClient for ArtifactDownloadClient {
    async fn open(
        &self,
        source: &ArtifactSource,
        offset: u64,
        expected_size: u64,
    ) -> Result<Box<dyn ArtifactBody>, ArtifactSourceError> {
        let stream = match source {
            ArtifactSource::HuggingFace {
                repository,
                revision,
                path,
            } => match lettuce_model_hub::pinned_resolve_url(repository, revision, path) {
                Ok(url) => self.open_hugging_face(&url, offset, expected_size).await,
                Err(_) => Err(ArtifactDownloadError::InvalidRequest),
            },
            ArtifactSource::Https { url } => self.open_https(url, offset, expected_size).await,
        };
        stream
            .map(|stream| Box::new(stream) as Box<dyn ArtifactBody>)
            .map_err(map_download_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactInstallAdmission {
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug)]
pub struct ArtifactInstallClaimedWork {
    pub plan: ArtifactInstallPlan,
    pub lease_for: Duration,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug)]
pub enum ArtifactInstallRunResult {
    Succeeded {
        paths: Vec<PathBuf>,
        job: JobSnapshot,
    },
    Cancelled {
        job: JobSnapshot,
    },
    Failed {
        error: ArtifactInstallError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactInstallError {
    #[error("artifact install storage failed: {0}")]
    Store(#[from] PinnedArtifactError),
    #[error("artifact download failed: {0}")]
    Source(#[from] ArtifactSourceError),
    #[error("artifact install job failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("artifact install work is inconsistent")]
    InvalidWork,
    #[error("artifact install was cancelled")]
    Cancelled,
    #[error("artifact install could not be finished: {0}")]
    Finish(String),
}

/// Runs installs inline. A crash leaves the job interrupted and the partial
/// bytes on disk; installing the same plan again resumes them.
#[derive(Debug)]
pub struct ArtifactInstallCoordinator<'a, J: ?Sized> {
    jobs: &'a J,
}

impl<'a, J: ?Sized> ArtifactInstallCoordinator<'a, J> {
    #[must_use]
    pub const fn new(jobs: &'a J) -> Self {
        Self { jobs }
    }
}

impl<J: JobStore + ?Sized> ArtifactInstallCoordinator<'_, J> {
    pub fn admit(
        &self,
        plan: &ArtifactInstallPlan,
    ) -> Result<ArtifactInstallAdmission, ArtifactInstallError> {
        self.admit_with_display(plan, None)
    }

    /// Like [`Self::admit`], with `label` as the job subject's display, so
    /// every install of one family (any revision) can be found in the job
    /// store by that label.
    pub fn admit_labeled(
        &self,
        plan: &ArtifactInstallPlan,
        label: &str,
    ) -> Result<ArtifactInstallAdmission, ArtifactInstallError> {
        self.admit_with_display(plan, Some(label))
    }

    fn admit_with_display(
        &self,
        plan: &ArtifactInstallPlan,
        display: Option<&str>,
    ) -> Result<ArtifactInstallAdmission, ArtifactInstallError> {
        if plan.artifacts.is_empty() {
            return Err(ArtifactInstallError::InvalidWork);
        }
        let asset_id = plan.outcome_asset_id();
        let mut subject = JobSubject::new(SubjectKind::ArtifactInstall, asset_id.to_string())
            .map_err(|_| ArtifactInstallError::InvalidWork)?;
        if let Some(display) = display {
            subject = subject
                .with_display(display)
                .map_err(|_| ArtifactInstallError::InvalidWork)?;
        }
        let mut earlier = 0_usize;
        let mut page = PageRequest::default();
        loop {
            let listed = self.jobs.list(JobQuery {
                kind: Some(JobKind::ArtifactInstall),
                subject: Some(subject.id.clone()),
                page,
                ..JobQuery::default()
            })?;
            if let Some(active) = listed.items.iter().find(|job| !job.state.is_terminal()) {
                return Ok(ArtifactInstallAdmission {
                    job: active.clone(),
                    created: false,
                });
            }
            earlier += listed.items.len();
            match listed.next_cursor {
                Some(cursor) => {
                    page = PageRequest {
                        cursor: Some(cursor),
                        ..PageRequest::default()
                    };
                }
                None => break,
            }
        }
        let admitted = self.jobs.create_or_get(
            JobSpec::new(
                JobKind::ArtifactInstall,
                subject,
                OutcomeRef::ArtifactInstallation(asset_id),
            )
            .with_idempotency_key(
                IdempotencyKey::new(format!("artifact-install-{asset_id}-{earlier}"))
                    .map_err(|_| ArtifactInstallError::InvalidWork)?,
            )
            .with_priority(JobPriority::Interactive)
            .with_resources(vec![
                ResourceClass::Network,
                ResourceClass::DiskRead,
                ResourceClass::DiskWrite,
                ResourceClass::Cpu,
            ])
            .with_policies(
                RecoveryPolicy::MarkInterrupted,
                CancellationPolicy::Cooperative,
            ),
        )?;
        Ok(ArtifactInstallAdmission {
            job: admitted.job,
            created: admitted.created,
        })
    }

    pub fn claim(
        &self,
        plan: ArtifactInstallPlan,
        job_id: JobId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<ArtifactInstallClaimedWork>, ArtifactInstallError> {
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(ArtifactInstallError::InvalidWork)?;
        validate_job(&job, &plan)?;
        if job.state.is_terminal() {
            return Ok(None);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self.jobs.claim(job_id, worker_id, at, lease_for, allowed)? else {
            return Ok(None);
        };
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("download", false).expect("constant stage"),
            at,
        })?;
        Ok(Some(ArtifactInstallClaimedWork {
            plan,
            lease_for,
            claim,
            handle: JobHandle::new(job_id),
            job,
        }))
    }

    pub async fn run<S: ArtifactSourceClient + ?Sized>(
        &self,
        work: ArtifactInstallClaimedWork,
        source: &S,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<ArtifactInstallRunResult, ArtifactInstallError> {
        validate_job(&work.job, &work.plan)?;
        if work.job.state != JobState::Running
            || work.claim.claim.job_id != work.job.id
            || work.handle.id() != work.job.id
        {
            return Err(ArtifactInstallError::InvalidWork);
        }
        let mut at = now.max(work.job.updated_at);
        let outcome = self.execute(&work, source, &mut at).await;
        self.settle(work, outcome, cancellation_reason, at)
    }

    /// Like [`Self::run`], but `finish` completes the install (such as
    /// unpacking a downloaded archive) as the job's `install` stage before
    /// the job succeeds. It receives the downloaded files and the job's
    /// cancellation token; its error fails the job, and
    /// [`ArtifactInstallError::Cancelled`] cancels it.
    pub async fn run_then<S, F, Fut>(
        &self,
        work: ArtifactInstallClaimedWork,
        source: &S,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
        finish: F,
    ) -> Result<ArtifactInstallRunResult, ArtifactInstallError>
    where
        S: ArtifactSourceClient + ?Sized,
        F: FnOnce(Vec<PathBuf>, CancellationToken) -> Fut,
        Fut: std::future::Future<Output = Result<(), ArtifactInstallError>>,
    {
        validate_job(&work.job, &work.plan)?;
        if work.job.state != JobState::Running
            || work.claim.claim.job_id != work.job.id
            || work.handle.id() != work.job.id
        {
            return Err(ArtifactInstallError::InvalidWork);
        }
        let mut at = now.max(work.job.updated_at);
        let outcome = match self.execute(&work, source, &mut at).await {
            Ok(paths) => {
                at = at.max(TimestampMillis::now().unwrap_or(at));
                match self.jobs.append_and_transition(JobMutation::StageChanged {
                    claim: work.claim.claim.clone(),
                    stage: StageSnapshot::new("install", false).expect("constant stage"),
                    at,
                }) {
                    Ok(_) => self
                        .finish_with_lease(
                            &work,
                            finish(paths.clone(), work.handle.cancellation_token()),
                            &mut at,
                        )
                        .await
                        .map(|()| paths),
                    Err(error) => Err(error.into()),
                }
            }
            Err(error) => Err(error),
        };
        at = at.max(TimestampMillis::now().unwrap_or(at));
        self.settle(work, outcome, cancellation_reason, at)
    }

    /// Awaits `finishing` while renewing the claim's lease, so a long
    /// install stage is not recovered as interrupted while it runs.
    async fn finish_with_lease<Fut>(
        &self,
        work: &ArtifactInstallClaimedWork,
        finishing: Fut,
        at: &mut TimestampMillis,
    ) -> Result<(), ArtifactInstallError>
    where
        Fut: std::future::Future<Output = Result<(), ArtifactInstallError>>,
    {
        let renew_every = (work.lease_for / 3).max(Duration::from_millis(1));
        tokio::pin!(finishing);
        loop {
            tokio::select! {
                finished = &mut finishing => return finished,
                () = tokio::time::sleep(renew_every) => {
                    *at = (*at).max(TimestampMillis::now().unwrap_or(*at));
                    if let Err(error) =
                        self.jobs.heartbeat(&work.claim.claim, *at, work.lease_for)
                    {
                        tracing::warn!(
                            install = %work.plan.install_id,
                            %error,
                            "could not renew the artifact install lease"
                        );
                    }
                }
            }
        }
    }

    /// Closes a job whose files were installed although its outcome could
    /// not be recorded: records success, else a failure, so the job does
    /// not stay running until its lease lapses. Failures are only logged.
    pub fn close_installed(&self, claim: &lettuce_jobs::ClaimRef, plan: &ArtifactInstallPlan) {
        let job = match self.jobs.get(claim.job_id) {
            Ok(Some(job)) if !job.state.is_terminal() => job,
            Ok(_) => return,
            Err(error) => {
                tracing::warn!(install = %plan.install_id, %error, "could not read the installed artifact job");
                return;
            }
        };
        let at = TimestampMillis::now()
            .unwrap_or(job.updated_at)
            .max(job.updated_at);
        let succeeded = self.jobs.append_and_transition(JobMutation::Succeed {
            claim: claim.clone(),
            outcome: JobOutcome::Success {
                result_ref: OutcomeRef::ArtifactInstallation(plan.outcome_asset_id()),
            },
            at,
        });
        let Err(error) = succeeded else {
            return;
        };
        tracing::warn!(install = %plan.install_id, %error, "could not record the installed artifact job");
        let failed = self.jobs.append_and_transition(JobMutation::Fail {
            claim: claim.clone(),
            error: JobError::new(
                JobErrorCode::ResourceUnavailable,
                false,
                "artifact installed but not recorded",
            )
            .expect("constant error label"),
            at,
        });
        if let Err(error) = failed {
            tracing::warn!(install = %plan.install_id, %error, "could not close the installed artifact job");
        }
    }

    fn settle(
        &self,
        work: ArtifactInstallClaimedWork,
        outcome: Result<Vec<PathBuf>, ArtifactInstallError>,
        cancellation_reason: CancellationReason,
        at: TimestampMillis,
    ) -> Result<ArtifactInstallRunResult, ArtifactInstallError> {
        match outcome {
            Ok(paths) => {
                let job = self.jobs.append_and_transition(JobMutation::Succeed {
                    claim: work.claim.claim,
                    outcome: JobOutcome::Success {
                        result_ref: OutcomeRef::ArtifactInstallation(work.plan.outcome_asset_id()),
                    },
                    at,
                })?;
                Ok(ArtifactInstallRunResult::Succeeded { paths, job })
            }
            Err(ArtifactInstallError::Cancelled) => {
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
                Ok(ArtifactInstallRunResult::Cancelled { job })
            }
            Err(error) => {
                tracing::warn!(install = %work.plan.install_id, %error, "artifact install failed");
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: JobError::new(
                        JobErrorCode::ResourceUnavailable,
                        false,
                        "artifact install failed",
                    )
                    .expect("constant error label"),
                    at,
                })?;
                Ok(ArtifactInstallRunResult::Failed { error, job })
            }
        }
    }

    /// Every file stays cancellable; files already verified stay installed
    /// and are reused by the next install. A cancelled or failed file leaves
    /// no partial behind, since the job fails for good instead of retrying.
    async fn execute<S: ArtifactSourceClient + ?Sized>(
        &self,
        work: &ArtifactInstallClaimedWork,
        source: &S,
        at: &mut TimestampMillis,
    ) -> Result<Vec<PathBuf>, ArtifactInstallError> {
        check_cancelled(&work.handle)?;
        let store = PinnedArtifactStore::open(&work.plan.root)?;
        let total = work.plan.total_bytes();
        let mut completed = 0_u64;
        let mut paths = Vec::with_capacity(work.plan.artifacts.len());
        for planned in &work.plan.artifacts {
            check_cancelled(&work.handle)?;
            match store.prepare(planned.artifact.clone())? {
                PinnedArtifactPreparation::Installed(path) => {
                    completed += planned.artifact.byte_size;
                    self.report_progress(work, completed, total, at)?;
                    paths.push(path);
                }
                PinnedArtifactPreparation::Download(mut download) => {
                    if let Err(error) = self
                        .download(work, source, planned, &mut download, (completed, total), at)
                        .await
                    {
                        if let Err(discard) = download.discard() {
                            tracing::warn!(error = %discard, "failed to remove a partial download");
                        }
                        return Err(error);
                    }
                    check_cancelled(&work.handle)?;
                    paths.push(download.finish()?);
                    completed += planned.artifact.byte_size;
                }
            }
        }
        Ok(paths)
    }

    async fn download<S: ArtifactSourceClient + ?Sized>(
        &self,
        work: &ArtifactInstallClaimedWork,
        source: &S,
        planned: &PlannedArtifact,
        download: &mut PinnedDownload,
        (completed_before, total): (u64, u64),
        at: &mut TimestampMillis,
    ) -> Result<(), ArtifactInstallError> {
        let size = planned.artifact.byte_size;
        if download.offset() == size {
            return Ok(());
        }
        let mut body = source
            .open(&planned.source, download.offset(), size)
            .await?;
        if body.start() != download.offset() {
            if body.start() != 0 {
                return Err(ArtifactSourceError::InvalidResponse.into());
            }
            download.restart()?;
        }
        let mut reported = download.offset();
        self.report_progress(work, completed_before + reported, total, at)?;
        while let Some(chunk) = body.next_chunk().await? {
            check_cancelled(&work.handle)?;
            let offset = download.append(&chunk)?;
            if offset == size || offset.saturating_sub(reported) >= PROGRESS_INTERVAL_BYTES {
                self.report_progress(work, completed_before + offset, total, at)?;
                reported = offset;
            }
        }
        if download.offset() != size {
            return Err(ArtifactSourceError::Transport.into());
        }
        Ok(())
    }

    /// Records progress and renews the lease so a long download is not
    /// recovered as interrupted while it runs.
    fn report_progress(
        &self,
        work: &ArtifactInstallClaimedWork,
        completed: u64,
        total: u64,
        at: &mut TimestampMillis,
    ) -> Result<(), ArtifactInstallError> {
        *at = (*at).max(TimestampMillis::now().unwrap_or(*at));
        self.jobs
            .heartbeat(&work.claim.claim, *at, work.lease_for)?;
        self.jobs.append_and_transition(JobMutation::Progress {
            claim: work.claim.claim.clone(),
            progress: ProgressSnapshot {
                bytes: Some(
                    BytesProgress::new(completed.min(total), Some(total))
                        .map_err(|_| ArtifactInstallError::InvalidWork)?,
                ),
                ..ProgressSnapshot::default()
            },
            at: *at,
        })?;
        Ok(())
    }
}

fn check_cancelled(handle: &JobHandle) -> Result<(), ArtifactInstallError> {
    if handle.cancellation_token().is_cancelled() {
        Err(ArtifactInstallError::Cancelled)
    } else {
        Ok(())
    }
}

fn validate_job(job: &JobSnapshot, plan: &ArtifactInstallPlan) -> Result<(), ArtifactInstallError> {
    let asset_id = plan.outcome_asset_id();
    if job.kind != JobKind::ArtifactInstall
        || job.subject.kind != SubjectKind::ArtifactInstall
        || job.subject.id.as_str() != asset_id.to_string()
    {
        return Err(ArtifactInstallError::InvalidWork);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use lettuce_database::Database;
    use lettuce_types::OperationId;
    use sha2::{Digest, Sha256};

    use super::*;

    struct Body {
        bytes: Vec<u8>,
        start: u64,
        sent: bool,
    }

    #[async_trait]
    impl ArtifactBody for Body {
        fn start(&self) -> u64 {
            self.start
        }

        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ArtifactSourceError> {
            if self.sent {
                return Ok(None);
            }
            self.sent = true;
            Ok(Some(
                self.bytes[usize::try_from(self.start).expect("offset")..].to_vec(),
            ))
        }
    }

    struct Source {
        files: Vec<(ArtifactSource, Vec<u8>)>,
        opens: Mutex<Vec<(ArtifactSource, u64)>>,
    }

    #[async_trait]
    impl ArtifactSourceClient for Source {
        async fn open(
            &self,
            source: &ArtifactSource,
            offset: u64,
            _expected_size: u64,
        ) -> Result<Box<dyn ArtifactBody>, ArtifactSourceError> {
            self.opens
                .lock()
                .expect("opens")
                .push((source.clone(), offset));
            let (_, bytes) = self
                .files
                .iter()
                .find(|(candidate, _)| candidate == source)
                .ok_or(ArtifactSourceError::InvalidRequest)?;
            Ok(Box::new(Body {
                bytes: bytes.clone(),
                start: offset,
                sent: false,
            }))
        }
    }

    fn planned(source: ArtifactSource, segments: &[&str], bytes: &[u8]) -> PlannedArtifact {
        PlannedArtifact {
            artifact: PinnedArtifact {
                source_identity: source.identity(),
                local_segments: segments.iter().map(|&segment| segment.to_owned()).collect(),
                byte_size: bytes.len() as u64,
                sha256: Some(format!("{:x}", Sha256::digest(bytes))),
            },
            source,
        }
    }

    /// The install stage outlasts the original lease and only ends after it
    /// observed the lease being renewed, so the outcome does not depend on
    /// how quickly the stage runs.
    #[tokio::test]
    async fn the_install_stage_renews_the_lease_and_decides_the_outcome() {
        let root = std::env::temp_dir().join(format!("artifact-finish-{}", OperationId::new()));
        let database = Database::open_in_memory().expect("database");
        let archive = ArtifactSource::Https {
            url: "https://github.com/owner/repo/releases/download/v1/engine.zip".to_owned(),
        };
        let source = Source {
            files: vec![(archive.clone(), b"zip bytes".to_vec())],
            opens: Mutex::new(Vec::new()),
        };
        const LEASE: Duration = Duration::from_secs(3);
        let coordinator = ArtifactInstallCoordinator::new(&database);
        for (install_id, fails) in [("finish:ok", false), ("finish:fails", true)] {
            let plan = ArtifactInstallPlan {
                install_id: install_id.to_owned(),
                root: root.join(install_id.replace(':', "-")),
                artifacts: vec![planned(archive.clone(), &["engine.zip"], b"zip bytes")],
            };
            let admitted = coordinator.admit(&plan).expect("admit");
            let job_id = admitted.job.id;
            let database = &database;
            let now = TimestampMillis::now().expect("now");
            let work = coordinator
                .claim(
                    plan,
                    admitted.job.id,
                    WorkerId::new(),
                    now,
                    LEASE,
                    &ResourceAvailability::all(),
                )
                .expect("claim")
                .expect("work");
            let result = coordinator
                .run_then(
                    work,
                    &source,
                    CancellationReason::User,
                    now,
                    |paths, _| async move {
                        assert!(paths[0].ends_with("engine.zip"));
                        let started = std::time::Instant::now();
                        let expiry = || {
                            database
                                .get(job_id)
                                .expect("job")
                                .expect("present")
                                .lease_expires_at
                        };
                        let mut last = expiry();
                        let mut renewals = 0;
                        while renewals < 3 || started.elapsed() <= LEASE {
                            assert!(
                                started.elapsed() < Duration::from_secs(60),
                                "the lease was not renewed"
                            );
                            tokio::time::sleep(Duration::from_millis(5)).await;
                            let current = expiry();
                            if current > last {
                                renewals += 1;
                                last = current;
                            }
                        }
                        if fails {
                            Err(ArtifactInstallError::Finish("unpack failed".to_owned()))
                        } else {
                            Ok(())
                        }
                    },
                )
                .await
                .expect("the lease outlives the install stage");
            match (fails, result) {
                (false, ArtifactInstallRunResult::Succeeded { job, .. }) => {
                    assert_eq!(job.state, JobState::Succeeded);
                }
                (true, ArtifactInstallRunResult::Failed { error, job }) => {
                    assert!(matches!(error, ArtifactInstallError::Finish(_)));
                    assert_eq!(job.state, JobState::Failed);
                }
                (_, other) => panic!("unexpected result: {other:?}"),
            }
        }
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn installs_every_artifact_once_and_reuses_verified_files() {
        let root = std::env::temp_dir().join(format!("artifact-install-{}", OperationId::new()));
        let database = Database::open_in_memory().expect("database");
        let model = ArtifactSource::HuggingFace {
            repository: "owner/repo".to_owned(),
            revision: "a".repeat(40),
            path: "split_files/vae/ae.safetensors".to_owned(),
        };
        let archive = ArtifactSource::Https {
            url: "https://github.com/owner/repo/releases/download/v1/engine.zip".to_owned(),
        };
        let source = Source {
            files: vec![
                (model.clone(), b"vae bytes".to_vec()),
                (archive.clone(), b"zip bytes".to_vec()),
            ],
            opens: Mutex::new(Vec::new()),
        };
        let plan = ArtifactInstallPlan {
            install_id: "sdcpp:test".to_owned(),
            root: root.clone(),
            artifacts: vec![
                planned(model, &["components", "ab", "ae.safetensors"], b"vae bytes"),
                planned(archive, &["v1", "engine.zip"], b"zip bytes"),
            ],
        };
        let coordinator = ArtifactInstallCoordinator::new(&database);
        for expected_opens in [2, 2] {
            let admitted = coordinator.admit(&plan).expect("admit");
            let work = coordinator
                .claim(
                    plan.clone(),
                    admitted.job.id,
                    WorkerId::new(),
                    TimestampMillis::new(1),
                    Duration::from_secs(30),
                    &ResourceAvailability::all(),
                )
                .expect("claim")
                .expect("work");
            let result = coordinator
                .run(
                    work,
                    &source,
                    CancellationReason::User,
                    TimestampMillis::new(2),
                )
                .await
                .expect("run");
            let ArtifactInstallRunResult::Succeeded { paths, job } = result else {
                panic!("expected success: {result:?}");
            };
            assert_eq!(job.state, JobState::Succeeded);
            assert_eq!(
                std::fs::read(&paths[0]).expect("vae"),
                b"vae bytes".to_vec()
            );
            assert!(paths[1].ends_with("v1/engine.zip"));
            assert_eq!(source.opens.lock().expect("opens").len(), expected_opens);
        }
        std::fs::remove_dir_all(root).ok();
    }
}
