use std::time::Duration;

use async_trait::async_trait;
use lettuce_jobs::{
    BytesProgress, CancellationPolicy, CancellationReason, Claim, IdempotencyKey, JobError,
    JobErrorCode, JobKind, JobMutation, JobOutcome, JobPriority, JobSnapshot, JobState, JobStore,
    JobSubject, OutcomeRef, ProgressSnapshot, RecoveryPolicy, ResourceAvailability, ResourceClass,
    StageSnapshot, StoreError, SubjectKind, WorkerId, handle::JobHandle,
};
use lettuce_model_hub::{
    InstalledKokoroVoice, KOKORO_REPOSITORY, KokoroInstallError, KokoroVoiceDownloadSession,
    KokoroVoiceInstallStore, KokoroVoicePreparation, RemoteKokoroVoice,
};
use lettuce_network::{ArtifactDownloadClient, ArtifactDownloadError, ArtifactDownloadStream};
use lettuce_types::{AssetId, JobId, TimestampMillis};
use uuid::Uuid;

use crate::{KokoroArtifactSummary, KokoroAvailableVoice, KokoroInstalledVoiceSummary};

const MAX_VOICES_PER_INSTALL: usize = 512;
const PROGRESS_INTERVAL_BYTES: u64 = 256 * 1024;
pub const KOKORO_STARTER_PACK_VOICE_IDS: [&str; 4] =
    ["af_heart", "am_adam", "bf_emma", "bm_george"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroVoiceBundle {
    pub voices: Vec<RemoteKokoroVoice>,
}

impl KokoroVoiceBundle {
    pub fn new(mut voices: Vec<RemoteKokoroVoice>) -> Result<Self, KokoroVoiceDownloadError> {
        if voices.is_empty() || voices.len() > MAX_VOICES_PER_INSTALL {
            return Err(KokoroVoiceDownloadError::InvalidWork);
        }
        for voice in &voices {
            voice.validate()?;
        }
        voices.sort_by(|left, right| left.id.cmp(&right.id));
        for pair in voices.windows(2) {
            if pair[0].id == pair[1].id && pair[0] != pair[1] {
                return Err(KokoroVoiceDownloadError::InvalidWork);
            }
        }
        voices.dedup();
        Ok(Self { voices })
    }

    pub fn from_catalog_ids(
        catalog: &[KokoroAvailableVoice],
        selected_ids: &[String],
    ) -> Result<Self, KokoroVoiceDownloadError> {
        let voices = selected_ids
            .iter()
            .map(|id| {
                catalog
                    .iter()
                    .find(|voice| voice.id == *id)
                    .ok_or(KokoroVoiceDownloadError::InvalidWork)?
                    .remote()
                    .map_err(|_| KokoroVoiceDownloadError::InvalidWork)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(voices)
    }

    pub fn starter_pack(
        catalog: &[KokoroAvailableVoice],
    ) -> Result<Self, KokoroVoiceDownloadError> {
        Self::from_catalog_ids(catalog, &KOKORO_STARTER_PACK_VOICE_IDS.map(str::to_owned))
    }

    fn validate(&self) -> Result<(), KokoroVoiceDownloadError> {
        if Self::new(self.voices.clone())? != *self {
            return Err(KokoroVoiceDownloadError::InvalidWork);
        }
        Ok(())
    }

    fn total_bytes(&self) -> Result<u64, KokoroVoiceDownloadError> {
        self.voices.iter().try_fold(0_u64, |total, voice| {
            total
                .checked_add(voice.byte_size)
                .ok_or(KokoroVoiceDownloadError::InvalidWork)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KokoroVoiceDownloadSourceError {
    #[error("Kokoro voice download transport failed")]
    Transport,
    #[error("Kokoro voice download response is invalid")]
    InvalidResponse,
}

#[async_trait]
pub trait KokoroVoiceDownloadBody: Send {
    fn start(&self) -> u64;
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, KokoroVoiceDownloadSourceError>;
}

#[async_trait]
pub trait KokoroVoiceDownloadSource: Send + Sync {
    async fn open(
        &self,
        voice: &RemoteKokoroVoice,
        offset: u64,
    ) -> Result<Box<dyn KokoroVoiceDownloadBody>, KokoroVoiceDownloadSourceError>;
}

#[derive(Debug, Clone)]
pub struct HuggingFaceKokoroVoiceDownloadSource {
    client: ArtifactDownloadClient,
}

impl HuggingFaceKokoroVoiceDownloadSource {
    pub fn new() -> Result<Self, ArtifactDownloadError> {
        ArtifactDownloadClient::new().map(|client| Self { client })
    }
}

#[async_trait]
impl KokoroVoiceDownloadSource for HuggingFaceKokoroVoiceDownloadSource {
    async fn open(
        &self,
        voice: &RemoteKokoroVoice,
        offset: u64,
    ) -> Result<Box<dyn KokoroVoiceDownloadBody>, KokoroVoiceDownloadSourceError> {
        self.client
            .open_hugging_face(
                KOKORO_REPOSITORY,
                &voice.source_revision,
                &voice.remote_path,
                offset,
                voice.byte_size,
            )
            .await
            .map(|stream| Box::new(stream) as Box<dyn KokoroVoiceDownloadBody>)
            .map_err(map_source_error)
    }
}

#[async_trait]
impl KokoroVoiceDownloadBody for ArtifactDownloadStream {
    fn start(&self) -> u64 {
        ArtifactDownloadStream::start(self)
    }

    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, KokoroVoiceDownloadSourceError> {
        ArtifactDownloadStream::next_chunk(self)
            .await
            .map_err(map_source_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroVoiceDownloadAdmission {
    pub bundle: KokoroVoiceBundle,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug)]
pub struct KokoroVoiceDownloadClaimedWork {
    pub bundle: KokoroVoiceBundle,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroVoiceDownloadSuccess {
    pub voices: Vec<KokoroInstalledVoiceSummary>,
    pub job: JobSnapshot,
    pub replayed: bool,
}

#[derive(Debug)]
pub enum KokoroVoiceDownloadRunResult {
    Succeeded(KokoroVoiceDownloadSuccess),
    Cancelled {
        error: KokoroVoiceDownloadError,
        job: JobSnapshot,
    },
    Failed {
        error: KokoroVoiceDownloadError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: KokoroVoiceDownloadError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum KokoroVoiceDownloadError {
    #[error("Kokoro voice installation failed: {0}")]
    Install(#[from] KokoroInstallError),
    #[error("Kokoro voice download failed: {0}")]
    Source(#[from] KokoroVoiceDownloadSourceError),
    #[error("Kokoro voice job failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("Kokoro voice download work is inconsistent")]
    InvalidWork,
    #[error("Kokoro voice download was cancelled")]
    Cancelled,
}

#[derive(Debug)]
pub struct KokoroVoiceDownloadCoordinator<'a, J: ?Sized> {
    jobs: &'a J,
    installs: KokoroVoiceInstallStore,
}

impl<'a, J: ?Sized> KokoroVoiceDownloadCoordinator<'a, J> {
    #[must_use]
    pub const fn new(jobs: &'a J, installs: KokoroVoiceInstallStore) -> Self {
        Self { jobs, installs }
    }
}

impl<J: JobStore + ?Sized> KokoroVoiceDownloadCoordinator<'_, J> {
    pub fn admit(
        &self,
        bundle: KokoroVoiceBundle,
    ) -> Result<KokoroVoiceDownloadAdmission, KokoroVoiceDownloadError> {
        bundle.validate()?;
        let asset_id = download_asset_id(&bundle);
        let subject = JobSubject::new(SubjectKind::ArtifactInstall, asset_id.to_string())
            .map_err(|_| KokoroVoiceDownloadError::InvalidWork)?;
        let mut key = format!("kokoro-voices-{asset_id}");
        for _ in 0..64 {
            let admitted = self.jobs.create_or_get(
                lettuce_jobs::JobSpec::new(
                    JobKind::ArtifactInstall,
                    subject.clone(),
                    OutcomeRef::ArtifactInstallation(asset_id),
                )
                .with_idempotency_key(
                    IdempotencyKey::new(key).map_err(|_| KokoroVoiceDownloadError::InvalidWork)?,
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
            validate_job(&admitted.job, &bundle)?;
            if admitted.job.state != JobState::Succeeded
                || self.installs.installed(&bundle.voices)?.is_some()
            {
                return Ok(KokoroVoiceDownloadAdmission {
                    bundle,
                    job: admitted.job,
                    created: admitted.created,
                });
            }
            key = format!("kokoro-voices-{asset_id}-after-{}", admitted.job.id);
        }
        Err(KokoroVoiceDownloadError::InvalidWork)
    }

    pub fn claim(
        &self,
        bundle: KokoroVoiceBundle,
        job_id: JobId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<KokoroVoiceDownloadClaimedWork>, KokoroVoiceDownloadError> {
        bundle.validate()?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(KokoroVoiceDownloadError::InvalidWork)?;
        validate_job(&job, &bundle)?;
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
        Ok(Some(KokoroVoiceDownloadClaimedWork {
            bundle,
            claim,
            handle,
            job,
        }))
    }

    pub fn replay(
        &self,
        bundle: &KokoroVoiceBundle,
        job_id: JobId,
    ) -> Result<Option<KokoroVoiceDownloadSuccess>, KokoroVoiceDownloadError> {
        bundle.validate()?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(KokoroVoiceDownloadError::InvalidWork)?;
        validate_job(&job, bundle)?;
        if job.state != JobState::Succeeded {
            return Ok(None);
        }
        let installed = self
            .installs
            .installed(&bundle.voices)?
            .ok_or(KokoroVoiceDownloadError::InvalidWork)?;
        Ok(Some(KokoroVoiceDownloadSuccess {
            voices: summaries(installed),
            job,
            replayed: true,
        }))
    }

    pub async fn run<S: KokoroVoiceDownloadSource + ?Sized>(
        &self,
        work: KokoroVoiceDownloadClaimedWork,
        source: &S,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<KokoroVoiceDownloadRunResult, KokoroVoiceDownloadError> {
        validate_job(&work.job, &work.bundle)?;
        if work.job.state != JobState::Running
            || work.claim.claim.job_id != work.job.id
            || work.handle.id() != work.job.id
        {
            return Err(KokoroVoiceDownloadError::InvalidWork);
        }
        match self.execute(&work, source, now).await {
            Ok((voices, replayed)) => self.finish_success(work, voices, replayed, now),
            Err(error) if matches!(error, KokoroVoiceDownloadError::Cancelled) => {
                self.finish_cancellation(work, error, cancellation_reason, now)
            }
            Err(error) => self.finish_error(work, error, now),
        }
    }

    async fn execute<S: KokoroVoiceDownloadSource + ?Sized>(
        &self,
        work: &KokoroVoiceDownloadClaimedWork,
        source: &S,
        now: TimestampMillis,
    ) -> Result<(Vec<KokoroInstalledVoiceSummary>, bool), KokoroVoiceDownloadError> {
        check_cancelled(&work.handle)?;
        if let Some(installed) = self.installs.installed(&work.bundle.voices)? {
            return Ok((summaries(installed), true));
        }
        let total = work.bundle.total_bytes()?;
        let mut completed = 0_u64;
        let mut irreversible = false;
        for voice in work.bundle.voices.clone() {
            if !irreversible {
                check_cancelled(&work.handle)?;
            }
            match self.installs.prepare(voice.clone())? {
                KokoroVoicePreparation::Installed(_) => {
                    completed = completed
                        .checked_add(voice.byte_size)
                        .ok_or(KokoroVoiceDownloadError::InvalidWork)?;
                    self.report_progress(work, completed, total, now)?;
                }
                KokoroVoicePreparation::Download(mut session) => {
                    self.download(
                        work,
                        source,
                        &voice,
                        &mut session,
                        completed,
                        total,
                        now,
                        !irreversible,
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
                    session.finish()?;
                    completed = completed
                        .checked_add(voice.byte_size)
                        .ok_or(KokoroVoiceDownloadError::InvalidWork)?;
                }
            }
        }
        let installed = self
            .installs
            .installed(&work.bundle.voices)?
            .ok_or(KokoroVoiceDownloadError::InvalidWork)?;
        Ok((summaries(installed), false))
    }

    #[expect(clippy::too_many_arguments, reason = "download state remains explicit")]
    async fn download<S: KokoroVoiceDownloadSource + ?Sized>(
        &self,
        work: &KokoroVoiceDownloadClaimedWork,
        source: &S,
        voice: &RemoteKokoroVoice,
        session: &mut KokoroVoiceDownloadSession,
        completed_before: u64,
        total: u64,
        now: TimestampMillis,
        cancellable: bool,
    ) -> Result<(), KokoroVoiceDownloadError> {
        if cancellable {
            check_cancelled(&work.handle)?;
        }
        let mut body = source.open(voice, session.offset()).await?;
        if body.start() != session.offset() {
            if body.start() != 0 {
                return Err(KokoroVoiceDownloadSourceError::InvalidResponse.into());
            }
            session.restart()?;
        }
        let mut reported = session.offset();
        self.report_progress(work, completed_before + reported, total, now)?;
        while let Some(chunk) = body.next_chunk().await? {
            if cancellable {
                check_cancelled(&work.handle)?;
            }
            let offset = session.append(&chunk)?;
            if offset == voice.byte_size
                || offset.saturating_sub(reported) >= PROGRESS_INTERVAL_BYTES
            {
                self.report_progress(work, completed_before + offset, total, now)?;
                reported = offset;
            }
        }
        if session.offset() != voice.byte_size {
            return Err(KokoroVoiceDownloadSourceError::Transport.into());
        }
        Ok(())
    }

    fn report_progress(
        &self,
        work: &KokoroVoiceDownloadClaimedWork,
        completed: u64,
        total: u64,
        now: TimestampMillis,
    ) -> Result<(), KokoroVoiceDownloadError> {
        self.jobs.append_and_transition(JobMutation::Progress {
            claim: work.claim.claim.clone(),
            progress: ProgressSnapshot {
                bytes: Some(
                    BytesProgress::new(completed, Some(total))
                        .map_err(|_| KokoroVoiceDownloadError::InvalidWork)?,
                ),
                ..ProgressSnapshot::default()
            },
            at: now.max(work.job.updated_at),
        })?;
        Ok(())
    }

    fn finish_success(
        &self,
        work: KokoroVoiceDownloadClaimedWork,
        voices: Vec<KokoroInstalledVoiceSummary>,
        replayed: bool,
        now: TimestampMillis,
    ) -> Result<KokoroVoiceDownloadRunResult, KokoroVoiceDownloadError> {
        let job = self.jobs.append_and_transition(JobMutation::Succeed {
            claim: work.claim.claim,
            outcome: JobOutcome::Success {
                result_ref: OutcomeRef::ArtifactInstallation(download_asset_id(&work.bundle)),
            },
            at: now.max(work.job.updated_at),
        })?;
        Ok(KokoroVoiceDownloadRunResult::Succeeded(
            KokoroVoiceDownloadSuccess {
                voices,
                job,
                replayed,
            },
        ))
    }

    fn finish_cancellation(
        &self,
        work: KokoroVoiceDownloadClaimedWork,
        error: KokoroVoiceDownloadError,
        reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<KokoroVoiceDownloadRunResult, KokoroVoiceDownloadError> {
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
        Ok(KokoroVoiceDownloadRunResult::Cancelled { error, job })
    }

    fn finish_error(
        &self,
        work: KokoroVoiceDownloadClaimedWork,
        error: KokoroVoiceDownloadError,
        now: TimestampMillis,
    ) -> Result<KokoroVoiceDownloadRunResult, KokoroVoiceDownloadError> {
        let at = now.max(work.job.updated_at);
        let (code, retryable, label) = classify_error(&error);
        if retryable {
            let job = self
                .jobs
                .append_and_transition(JobMutation::RetryScheduled {
                    claim: work.claim.claim,
                    at,
                })?;
            return Ok(KokoroVoiceDownloadRunResult::RetryScheduled { error, job });
        }
        let job = self.jobs.append_and_transition(JobMutation::Fail {
            claim: work.claim.claim,
            error: JobError::new(code, false, label).expect("constant error label"),
            at,
        })?;
        Ok(KokoroVoiceDownloadRunResult::Failed { error, job })
    }
}

fn summaries(voices: Vec<InstalledKokoroVoice>) -> Vec<KokoroInstalledVoiceSummary> {
    voices
        .into_iter()
        .map(|voice| KokoroInstalledVoiceSummary {
            id: voice.id,
            artifact: KokoroArtifactSummary {
                byte_size: voice.artifact.byte_size,
                blake3: voice.artifact.blake3,
            },
        })
        .collect()
}

fn check_cancelled(handle: &JobHandle) -> Result<(), KokoroVoiceDownloadError> {
    if handle.cancellation_token().is_cancelled() {
        Err(KokoroVoiceDownloadError::Cancelled)
    } else {
        Ok(())
    }
}

fn validate_job(
    job: &JobSnapshot,
    bundle: &KokoroVoiceBundle,
) -> Result<(), KokoroVoiceDownloadError> {
    let asset_id = download_asset_id(bundle);
    if job.kind != JobKind::ArtifactInstall
        || job.subject.kind != SubjectKind::ArtifactInstall
        || job.subject.id.as_str() != asset_id.to_string()
        || (job.state == JobState::Succeeded
            && job.outcome.as_ref()
                != Some(&JobOutcome::Success {
                    result_ref: OutcomeRef::ArtifactInstallation(asset_id),
                }))
    {
        return Err(KokoroVoiceDownloadError::InvalidWork);
    }
    Ok(())
}

fn download_asset_id(bundle: &KokoroVoiceBundle) -> AssetId {
    let mut identity = String::from("kokoro-voices");
    for voice in &bundle.voices {
        identity.push_str(&format!(
            "\0{}\0{}\0{}\0{}",
            voice.id, voice.source_revision, voice.byte_size, voice.sha256
        ));
    }
    AssetId::from_uuid(Uuid::new_v5(&Uuid::NAMESPACE_URL, identity.as_bytes()))
}

fn classify_error(error: &KokoroVoiceDownloadError) -> (JobErrorCode, bool, &'static str) {
    match error {
        KokoroVoiceDownloadError::Source(KokoroVoiceDownloadSourceError::Transport) => (
            JobErrorCode::ResourceUnavailable,
            true,
            "voice download unavailable",
        ),
        KokoroVoiceDownloadError::Install(KokoroInstallError::Platform(
            lettuce_platform::PlatformError::Io
            | lettuce_platform::PlatformError::RecoveryNeeded
            | lettuce_platform::PlatformError::ReplaceFailed,
        ))
        | KokoroVoiceDownloadError::Install(KokoroInstallError::Unreadable) => (
            JobErrorCode::StorageFailure,
            true,
            "voice storage unavailable",
        ),
        KokoroVoiceDownloadError::Install(
            KokoroInstallError::Mismatch | KokoroInstallError::InvalidArtifact,
        )
        | KokoroVoiceDownloadError::Source(KokoroVoiceDownloadSourceError::InvalidResponse) => (
            JobErrorCode::IntegrityFailure,
            false,
            "voice integrity failed",
        ),
        KokoroVoiceDownloadError::Cancelled => {
            (JobErrorCode::Cancelled, false, "voice download cancelled")
        }
        KokoroVoiceDownloadError::Jobs(StoreError::StaleLease | StoreError::LeaseExpired) => {
            (JobErrorCode::LeaseLost, true, "voice download lease lost")
        }
        _ => (JobErrorCode::InvalidInput, false, "voice input is invalid"),
    }
}

fn map_source_error(error: ArtifactDownloadError) -> KokoroVoiceDownloadSourceError {
    match error {
        ArtifactDownloadError::Transport => KokoroVoiceDownloadSourceError::Transport,
        ArtifactDownloadError::InvalidRequest | ArtifactDownloadError::InvalidResponse => {
            KokoroVoiceDownloadSourceError::InvalidResponse
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use super::*;
    use lettuce_database::Database;
    use lettuce_model_hub::KOKORO_SOURCE_REVISION;

    #[derive(Debug)]
    struct FixtureSource {
        fail_once: Mutex<bool>,
        offsets: Mutex<Vec<(String, u64)>>,
    }

    #[derive(Debug)]
    struct FixtureBody {
        start: u64,
        chunks: VecDeque<Result<Vec<u8>, KokoroVoiceDownloadSourceError>>,
    }

    #[async_trait]
    impl KokoroVoiceDownloadSource for FixtureSource {
        async fn open(
            &self,
            voice: &RemoteKokoroVoice,
            offset: u64,
        ) -> Result<Box<dyn KokoroVoiceDownloadBody>, KokoroVoiceDownloadSourceError> {
            self.offsets
                .lock()
                .expect("offsets")
                .push((voice.id.clone(), offset));
            let remaining = bytes_for(&voice.id)[offset as usize..].to_vec();
            let chunks = if *self.fail_once.lock().expect("failure flag") {
                *self.fail_once.lock().expect("failure flag") = false;
                let split = remaining.len().min(2);
                VecDeque::from([
                    Ok(remaining[..split].to_vec()),
                    Err(KokoroVoiceDownloadSourceError::Transport),
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
    impl KokoroVoiceDownloadBody for FixtureBody {
        fn start(&self) -> u64 {
            self.start
        }

        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, KokoroVoiceDownloadSourceError> {
            self.chunks.pop_front().transpose()
        }
    }

    fn remote(id: &str) -> RemoteKokoroVoice {
        let bytes = bytes_for(id);
        let sha256 = match id {
            "af_heart" => "3bc1af1ea0b65e55f5b755320c9b13e785e531dacdc77d11e4695412d6776272",
            "bm_george" => "d7534670de1eb571f3062ec2f6bd5a4e4dc3248b2ea8b5beb090f6022516bdfd",
            _ => panic!("unexpected voice"),
        };
        RemoteKokoroVoice::pinned(
            id,
            KOKORO_SOURCE_REVISION,
            u64::try_from(bytes.len()).expect("voice size"),
            sha256,
        )
        .expect("remote voice")
    }

    fn bytes_for(id: &str) -> &'static [u8] {
        match id {
            "af_heart" => b"heart voice",
            "bm_george" => b"george voice",
            _ => panic!("unexpected voice"),
        }
    }

    fn available(id: &str) -> KokoroAvailableVoice {
        let remote = RemoteKokoroVoice::pinned(id, KOKORO_SOURCE_REVISION, 42, "ab".repeat(32))
            .expect("remote voice");
        KokoroAvailableVoice {
            id: remote.id,
            installed: false,
            remote_path: remote.remote_path,
            source_revision: remote.source_revision,
            byte_size: remote.byte_size,
            sha256: remote.sha256,
        }
    }

    #[test]
    fn starter_pack_preserves_the_legacy_four_voice_selection() {
        let catalog = [
            available("bm_george"),
            available("bf_emma"),
            available("am_adam"),
            available("af_heart"),
        ];
        let bundle = KokoroVoiceBundle::starter_pack(&catalog).expect("starter pack");
        assert_eq!(
            bundle
                .voices
                .iter()
                .map(|voice| voice.id.as_str())
                .collect::<Vec<_>>(),
            KOKORO_STARTER_PACK_VOICE_IDS
        );
        assert!(KokoroVoiceBundle::starter_pack(&catalog[..3]).is_err());
    }

    #[tokio::test]
    async fn batch_deduplicates_resumes_after_reopen_and_replays_without_network() {
        let root = std::env::temp_dir().join(format!("kokoro-voice-job-{}", JobId::new()));
        let database_path = root.join("jobs.sqlite3");
        let install_root = root.join("assets");
        std::fs::create_dir_all(&root).expect("root");
        let source = FixtureSource {
            fail_once: Mutex::new(true),
            offsets: Mutex::new(Vec::new()),
        };
        let bundle = KokoroVoiceBundle::new(vec![
            remote("bm_george"),
            remote("af_heart"),
            remote("af_heart"),
        ])
        .expect("bundle");
        assert_eq!(bundle.voices.len(), 2);
        let job_id;
        {
            let database = Database::open(&database_path).expect("database");
            let installs = KokoroVoiceInstallStore::open(&install_root).expect("install store");
            let coordinator = KokoroVoiceDownloadCoordinator::new(&database, installs);
            let admitted = coordinator.admit(bundle.clone()).expect("admission");
            job_id = admitted.job.id;
            let work = coordinator
                .claim(
                    bundle.clone(),
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
                KokoroVoiceDownloadRunResult::RetryScheduled { .. }
            ));
        }
        let database = Database::open(&database_path).expect("reopen database");
        let installs = KokoroVoiceInstallStore::open(&install_root).expect("install store");
        let coordinator = KokoroVoiceDownloadCoordinator::new(&database, installs);
        let work = coordinator
            .claim(
                bundle.clone(),
                job_id,
                WorkerId::new(),
                TimestampMillis::new(12),
                Duration::from_secs(60),
                &ResourceAvailability::all(),
            )
            .expect("reclaim")
            .expect("work");
        let KokoroVoiceDownloadRunResult::Succeeded(success) = coordinator
            .run(
                work,
                &source,
                CancellationReason::User,
                TimestampMillis::new(13),
            )
            .await
            .expect("run")
        else {
            panic!("expected success");
        };
        assert_eq!(
            success
                .voices
                .iter()
                .map(|voice| voice.id.as_str())
                .collect::<Vec<_>>(),
            ["af_heart", "bm_george"]
        );
        let calls_before_replay = source.offsets.lock().expect("offsets").len();
        assert!(
            coordinator
                .replay(&bundle, job_id)
                .expect("replay")
                .is_some_and(|result| result.replayed)
        );
        assert_eq!(
            source.offsets.lock().expect("offsets").len(),
            calls_before_replay
        );
        assert_eq!(
            source.offsets.lock().expect("offsets")[1],
            ("af_heart".to_owned(), 2)
        );
        assert_eq!(
            std::fs::read(install_root.join("voices/bm_george.bin")).expect("voice"),
            bytes_for("bm_george")
        );
        let removal_store = KokoroVoiceInstallStore::open(&install_root).expect("removal store");
        assert!(
            crate::remove_managed_kokoro_voice(&removal_store, &bundle.voices[0])
                .expect("remove voice")
                .removed
        );
        assert!(!install_root.join("voices/af_heart.bin").exists());
        let replacement = coordinator
            .admit(bundle.clone())
            .expect("replacement admission");
        assert!(replacement.created);
        assert_ne!(replacement.job.id, job_id);
        drop(coordinator);
        drop(database);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[tokio::test]
    async fn cancellation_before_transfer_settles_without_voice_files() {
        let root = std::env::temp_dir().join(format!("kokoro-voice-cancel-{}", JobId::new()));
        let database = Database::open_in_memory().expect("database");
        let installs = KokoroVoiceInstallStore::open(&root).expect("install store");
        let coordinator = KokoroVoiceDownloadCoordinator::new(&database, installs);
        let bundle = KokoroVoiceBundle::new(vec![remote("af_heart")]).expect("bundle");
        let admitted = coordinator.admit(bundle.clone()).expect("admission");
        let work = coordinator
            .claim(
                bundle,
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
            KokoroVoiceDownloadRunResult::Cancelled { .. }
        ));
        assert!(source.offsets.lock().expect("offsets").is_empty());
        assert!(!root.join("voices/af_heart.bin").exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
