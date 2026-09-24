use std::time::Duration;

use lettuce_image_generation::sd_runtime::lora_library::{
    LoraLibraryRepository, LoraLibraryRepositoryError, hydrate_lora_keywords,
};
use lettuce_image_generation::{
    ImageGenerationRecord, ImageGenerationRepository, ImageGenerationRepositoryError,
    ImageGenerationRequest, ImageGenerationResult, ImageGenerationState, ImageInput, ImageMedia,
    ImageMediaError, ImageProfileError, ImageProviderError, ImageProviderPort,
    ImageRequestValidationError, ProviderImageRequest, ResolvedImageProfile, compose_image_prompt,
    lora_keywords, merge_loras, resolve_image_profile,
};
use lettuce_jobs::{
    CancellationPolicy, CancellationReason, Claim, IdempotencyKey, JobError, JobErrorCode, JobKind,
    JobMutation, JobOutcome, JobPriority, JobSnapshot, JobSpec, JobState, JobStore, JobSubject,
    OutcomeRef, RecoveryPolicy, ResourceAvailability, ResourceClass, StageSnapshot, StoreError,
    SubjectKind, WorkerId, handle::JobHandle,
};
use lettuce_models::{ModelProfileRepository, ModelRepositoryError, ProviderAccountRepository};
use lettuce_types::{GenerationAttemptId, JobId, TimestampMillis, UsageEventId};
use lettuce_usage::{JobInferenceUsage, JobInferenceUsageResult, JobUsageLedger};

#[derive(Debug, Clone, PartialEq)]
pub struct ImageGenerationAdmission {
    pub record: ImageGenerationRecord,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug)]
pub struct ImageGenerationClaimedWork {
    pub record: ImageGenerationRecord,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug)]
pub enum ImageGenerationRunResult {
    Succeeded {
        record: ImageGenerationRecord,
        job: JobSnapshot,
    },
    Failed {
        record: ImageGenerationRecord,
        job: JobSnapshot,
    },
    Cancelled {
        record: ImageGenerationRecord,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ImageGenerationError {
    #[error(transparent)]
    Invalid(#[from] ImageRequestValidationError),
    #[error("the image model no longer exists")]
    ModelMissing,
    #[error("the image model could not be read")]
    Models(ModelRepositoryError),
    #[error(transparent)]
    Profile(#[from] ImageProfileError),
    #[error(transparent)]
    Media(#[from] ImageMediaError),
    #[error(transparent)]
    Provider(#[from] ImageProviderError),
    #[error("image generation usage could not be recorded")]
    Usage,
    #[error("the LoRA library could not be read")]
    LoraLibrary(#[from] LoraLibraryRepositoryError),
    #[error("image generation persistence failed: {0}")]
    Repository(#[from] ImageGenerationRepositoryError),
    #[error("image generation job failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("image generation job and request are inconsistent")]
    InvalidWork,
}

/// Runs one image generation as an `ImageGenerate` job: the request is
/// admitted with its job, claimed, then executed inline by the caller.
#[derive(Debug)]
pub struct ImageGenerationCoordinator<'a, R: ?Sized, J: ?Sized> {
    generations: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> ImageGenerationCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(generations: &'a R, jobs: &'a J) -> Self {
        Self { generations, jobs }
    }
}

const INTERRUPTED_MESSAGE: &str = "Image generation was interrupted.";

/// The old playground's seed for local runs without one (0 to 2^31 - 1), drawn
/// from the request id so a replayed admission asks for the same seed.
fn playground_seed(request_id: lettuce_types::RequestId) -> u32 {
    let hash = blake3::hash(request_id.to_string().as_bytes());
    let bytes = hash.as_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) & 0x7fff_ffff
}

impl<
    R: ImageGenerationRepository + JobUsageLedger + LoraLibraryRepository + ?Sized,
    J: JobStore + ?Sized,
> ImageGenerationCoordinator<'_, R, J>
{
    pub fn admit<M>(
        &self,
        request: ImageGenerationRequest,
        models: &M,
    ) -> Result<ImageGenerationAdmission, ImageGenerationError>
    where
        M: ModelProfileRepository + ProviderAccountRepository + ?Sized,
    {
        request.validate()?;
        let profile = resolve(models, &request)?;
        let mut request = request;
        if request.source == lettuce_image_generation::ImageGenerationSource::Playground
            && profile.is_local_diffusion()
            && request.settings.seed.is_none()
        {
            request.settings.seed = Some(playground_seed(request.id));
        }
        let subject = JobSubject::new(SubjectKind::ImageRequest, request.id.to_string())
            .map_err(|_| ImageGenerationError::InvalidWork)?;
        let key = IdempotencyKey::new(format!("image-generate-{}", request.id))
            .map_err(|_| ImageGenerationError::InvalidWork)?;
        let resources = if profile.is_local_diffusion() {
            vec![
                ResourceClass::ModelLoad,
                ResourceClass::Gpu,
                ResourceClass::Process,
                ResourceClass::DiskWrite,
            ]
        } else {
            vec![ResourceClass::Network, ResourceClass::DiskWrite]
        };
        let admitted = self.jobs.create_or_get(
            JobSpec::new(
                JobKind::ImageGenerate,
                subject,
                OutcomeRef::Request(request.id),
            )
            .with_idempotency_key(key)
            .with_priority(JobPriority::Interactive)
            .with_resources(resources)
            .with_policies(
                RecoveryPolicy::MarkInterrupted,
                CancellationPolicy::Cooperative,
            ),
        )?;
        let record = self.generations.admit(ImageGenerationRecord {
            job_id: admitted.job.id,
            request,
            state: ImageGenerationState::Pending,
        })?;
        validate_job_record(&admitted.job, &record)?;
        let record = self.reconcile(&admitted.job, record)?;
        Ok(ImageGenerationAdmission {
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
    ) -> Result<Option<ImageGenerationClaimedWork>, ImageGenerationError> {
        let record = self.generations.get(job_id)?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(ImageGenerationError::InvalidWork)?;
        validate_job_record(&job, &record)?;
        if job.state.is_terminal() {
            self.reconcile(&job, record)?;
            return Ok(None);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self.jobs.claim(job_id, worker_id, at, lease_for, allowed)? else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::Request(record.request.id) {
            return Err(ImageGenerationError::InvalidWork);
        }
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("image-generation", false).expect("constant stage is valid"),
            at,
        })?;
        Ok(Some(ImageGenerationClaimedWork {
            record,
            claim,
            handle: JobHandle::new(job_id),
            job,
        }))
    }

    pub async fn run<M, D, P>(
        &self,
        work: ImageGenerationClaimedWork,
        models: &M,
        media: &D,
        provider: &P,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<ImageGenerationRunResult, ImageGenerationError>
    where
        M: ModelProfileRepository + ProviderAccountRepository + ?Sized,
        D: ImageMedia + ?Sized,
        P: ImageProviderPort + ?Sized,
    {
        validate_job_record(&work.job, &work.record)?;
        if work.job.state != JobState::Running
            || work.claim.claim.job_id != work.job.id
            || work.handle.id() != work.job.id
        {
            return Err(ImageGenerationError::InvalidWork);
        }
        if work.record.state != ImageGenerationState::Pending {
            return self.finish(work, cancellation_reason, now);
        }
        let state = match self.execute(&work, models, media, provider, now).await {
            Ok(result) => ImageGenerationState::Succeeded { result },
            Err(error)
                if matches!(
                    error,
                    ImageGenerationError::Provider(ImageProviderError::Cancelled)
                ) || work.handle.cancellation_token().is_cancelled() =>
            {
                ImageGenerationState::Cancelled {
                    completed_at: now.max(work.record.request.created_at),
                }
            }
            Err(error) => {
                tracing::warn!(
                    component = "image_generator",
                    error = %error,
                    "image generation failed"
                );
                ImageGenerationState::Failed {
                    message: error.to_string(),
                    completed_at: now.max(work.record.request.created_at),
                }
            }
        };
        let record = match self.generations.settle(work.job.id, state) {
            Ok(record) => record,
            Err(error) => {
                self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: JobError::new(
                        JobErrorCode::StorageFailure,
                        false,
                        "image generation could not be stored",
                    )
                    .expect("constant image error label is valid"),
                    at: now.max(work.job.updated_at),
                })?;
                return Err(error.into());
            }
        };
        self.finish(
            ImageGenerationClaimedWork { record, ..work },
            cancellation_reason,
            now,
        )
    }

    async fn execute<M, D, P>(
        &self,
        work: &ImageGenerationClaimedWork,
        models: &M,
        media: &D,
        provider: &P,
        now: TimestampMillis,
    ) -> Result<ImageGenerationResult, ImageGenerationError>
    where
        M: ModelProfileRepository + ProviderAccountRepository + ?Sized,
        D: ImageMedia + ?Sized,
        P: ImageProviderPort + ?Sized,
    {
        let request = &work.record.request;
        let cancellation = work.handle.cancellation_token();
        check_cancelled(&work.handle)?;
        let profile = resolve(models, request)?;
        let mut settings = profile.settings.overlaid_by(&request.settings);
        let mut request_loras = request.loras.clone();
        if profile.is_local_diffusion() {
            if let Some(base) = settings.base_loras.as_mut() {
                hydrate_lora_keywords(self.generations, base)?;
            }
            hydrate_lora_keywords(self.generations, &mut request_loras)?;
        }
        let loras = merge_loras(
            settings.base_loras.as_deref().unwrap_or_default(),
            &request_loras,
        );
        let prompt = compose_image_prompt(
            &request.prompt,
            settings.extra_prompt.as_deref(),
            &lora_keywords(&loras),
        );
        let usage_id = UsageEventId::new();
        self.generations
            .admit_job_usage(JobInferenceUsage {
                id: usage_id,
                job_id: work.job.id,
                logical_attempt_id: GenerationAttemptId::new(),
                model_profile_id: profile.model_profile_id,
                model_revision: profile.model_revision,
                provider_account_id: profile.account.id,
                provider_account_revision: profile.account.revision,
                admitted_at: now,
                result: None,
            })
            .map_err(|_| ImageGenerationError::Usage)?;
        let inputs = load_inputs(&work.handle, request, media);
        let output = match inputs {
            Ok((input_images, mask_image)) => provider
                .generate(ProviderImageRequest {
                    job_id: work.job.id,
                    model_profile_id: profile.model_profile_id,
                    account: profile.account.clone(),
                    external_model_id: profile.external_model_id.clone(),
                    model_display_name: profile.display_name.clone(),
                    prompt,
                    settings,
                    loras,
                    input_images,
                    mask_image,
                    size: request.size.clone(),
                    quality: request.quality.clone(),
                    style: request.style.clone(),
                    count: request.count,
                    text_output: profile.text_output,
                    cancellation,
                })
                .await
                .map_err(ImageGenerationError::Provider),
            Err(error) => Err(error),
        };
        let settled = match &output {
            Ok(output) => JobInferenceUsageResult::Response {
                usage: output.usage.clone(),
                provider_response_id: None,
            },
            Err(ImageGenerationError::Provider(ImageProviderError::Cancelled)) => {
                JobInferenceUsageResult::Cancelled
            }
            Err(_) => JobInferenceUsageResult::InferenceFailed,
        };
        if self
            .generations
            .settle_job_usage(usage_id, settled)
            .is_err()
        {
            tracing::warn!(
                component = "image_generator",
                "failed to record image generation usage"
            );
        }
        let output = output?;
        check_cancelled(&work.handle)?;
        let mut images = Vec::with_capacity(output.images.len());
        let mut rejected_outputs = 0_u32;
        let mut last_rejection = None;
        for image in output.images {
            match media.ingest_output(
                work.job.id,
                profile.model_profile_id,
                request.output_policy,
                image,
            ) {
                Ok(image) => images.push(image),
                Err(error) => {
                    rejected_outputs = rejected_outputs.saturating_add(1);
                    last_rejection = Some(error);
                }
            }
        }
        if images.is_empty() {
            return Err(match last_rejection {
                Some(error) => error.into(),
                None => {
                    ImageProviderError::Failed("No image URL or data in response.".into()).into()
                }
            });
        }
        Ok(ImageGenerationResult {
            request_id: request.id,
            images,
            rejected_outputs,
            completed_at: now.max(request.created_at),
        })
    }

    /// Settles the record of an ended job left pending; `None` while the job
    /// is still live.
    pub fn reconcile_after_restart(
        &self,
        job_id: JobId,
    ) -> Result<Option<ImageGenerationRecord>, ImageGenerationError> {
        let record = self.generations.get(job_id)?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(ImageGenerationError::InvalidWork)?;
        validate_job_record(&job, &record)?;
        if !job.state.is_terminal() {
            return Ok(None);
        }
        self.reconcile(&job, record).map(Some)
    }

    /// Settles the record of a job that ended without running to completion
    /// (interrupted by a crash or cancelled before it ran), together with any
    /// usage it admitted.
    fn reconcile(
        &self,
        job: &JobSnapshot,
        record: ImageGenerationRecord,
    ) -> Result<ImageGenerationRecord, ImageGenerationError> {
        if !job.state.is_terminal() || record.state != ImageGenerationState::Pending {
            return Ok(record);
        }
        for usage in self
            .generations
            .job_usage(job.id)
            .map_err(|_| ImageGenerationError::Usage)?
        {
            if usage.result.is_none() {
                self.generations
                    .settle_job_usage(usage.id, JobInferenceUsageResult::InferenceFailed)
                    .map_err(|_| ImageGenerationError::Usage)?;
            }
        }
        let completed_at = job.updated_at.max(record.request.created_at);
        let state = if job.state == JobState::Cancelled {
            ImageGenerationState::Cancelled { completed_at }
        } else {
            ImageGenerationState::Failed {
                message: INTERRUPTED_MESSAGE.to_owned(),
                completed_at,
            }
        };
        Ok(self.generations.settle(job.id, state)?)
    }

    fn finish(
        &self,
        work: ImageGenerationClaimedWork,
        reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<ImageGenerationRunResult, ImageGenerationError> {
        let at = now.max(work.job.updated_at);
        let claim = work.claim.claim;
        match &work.record.state {
            ImageGenerationState::Pending => Err(ImageGenerationError::InvalidWork),
            ImageGenerationState::Succeeded { .. } => {
                let job = self.jobs.append_and_transition(JobMutation::Succeed {
                    claim,
                    outcome: JobOutcome::Success {
                        result_ref: OutcomeRef::Request(work.record.request.id),
                    },
                    at,
                })?;
                Ok(ImageGenerationRunResult::Succeeded {
                    record: work.record,
                    job,
                })
            }
            ImageGenerationState::Failed { .. } => {
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim,
                    error: JobError::new(
                        JobErrorCode::WorkerFailed,
                        false,
                        "image generation failed",
                    )
                    .expect("constant image error label is valid"),
                    at,
                })?;
                Ok(ImageGenerationRunResult::Failed {
                    record: work.record,
                    job,
                })
            }
            ImageGenerationState::Cancelled { .. } => {
                self.jobs
                    .append_and_transition(JobMutation::RequestCancellation {
                        id: work.job.id,
                        reason,
                        at,
                    })?;
                self.jobs
                    .append_and_transition(JobMutation::RequestCleanup {
                        claim: claim.clone(),
                        at,
                    })?;
                let job = self
                    .jobs
                    .append_and_transition(JobMutation::FinishCancellation { claim, at })?;
                Ok(ImageGenerationRunResult::Cancelled {
                    record: work.record,
                    job,
                })
            }
        }
    }
}

fn load_inputs<D: ImageMedia + ?Sized>(
    handle: &JobHandle,
    request: &ImageGenerationRequest,
    media: &D,
) -> Result<(Vec<ImageInput>, Option<ImageInput>), ImageGenerationError> {
    let input_images = request
        .input_images
        .iter()
        .map(|&asset_id| media.load_input(asset_id))
        .collect::<Result<Vec<_>, _>>()?;
    let mask_image = request
        .mask_image
        .map(|asset_id| media.load_input(asset_id))
        .transpose()?;
    check_cancelled(handle)?;
    Ok((input_images, mask_image))
}

fn resolve<M>(
    models: &M,
    request: &ImageGenerationRequest,
) -> Result<ResolvedImageProfile, ImageGenerationError>
where
    M: ModelProfileRepository + ProviderAccountRepository + ?Sized,
{
    let profile = ModelProfileRepository::get(models, request.model_profile_id)
        .map_err(ImageGenerationError::Models)?
        .ok_or(ImageGenerationError::ModelMissing)?;
    let account = ProviderAccountRepository::get(models, profile.provider_account_id)
        .map_err(ImageGenerationError::Models)?
        .ok_or(ImageGenerationError::ModelMissing)?;
    Ok(resolve_image_profile(&profile, &account)?)
}

fn validate_job_record(
    job: &JobSnapshot,
    record: &ImageGenerationRecord,
) -> Result<(), ImageGenerationError> {
    record.validate()?;
    if job.id != record.job_id
        || job.kind != JobKind::ImageGenerate
        || job.subject.kind != SubjectKind::ImageRequest
        || job.subject.id.as_str() != record.request.id.to_string()
    {
        return Err(ImageGenerationError::InvalidWork);
    }
    if job.state == JobState::Succeeded
        && !matches!(record.state, ImageGenerationState::Succeeded { .. })
    {
        return Err(ImageGenerationError::InvalidWork);
    }
    Ok(())
}

fn check_cancelled(handle: &JobHandle) -> Result<(), ImageGenerationError> {
    if handle.cancellation_token().is_cancelled() {
        Err(ImageProviderError::Cancelled.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use lettuce_conversations::InferenceUsage;
    use lettuce_database::Database;
    use lettuce_image_generation::{
        ImageAttribution, ImageGenerationSource, ImageOutputPolicy, ProviderImage,
        ProviderImageOutput,
    };
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
        MediaAssetRepository, RetentionClass,
    };
    use lettuce_models::{
        CapabilityStatus, ModalityCapabilities, ModelCapabilities, ModelKind, ModelProfile,
        ModelProfileConfig, ProviderAccount, ProviderConfig, ProviderProtocol, StableDiffusionLora,
        StableDiffusionSettings,
    };
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_settings::{SecretOwnerId, SecretRef};
    use lettuce_types::{ModelProfileId, ProviderAccountId, RequestId, Revision};

    use super::*;

    const NOW: TimestampMillis = TimestampMillis::new(1_000);

    #[derive(Default)]
    struct Provider {
        requests: Mutex<Vec<ProviderImageRequest>>,
        outcome: Option<Result<Vec<ProviderImage>, ImageProviderError>>,
        cancel_during_call: bool,
    }

    #[async_trait]
    impl ImageProviderPort for Provider {
        async fn generate(
            &self,
            request: ProviderImageRequest,
        ) -> Result<ProviderImageOutput, ImageProviderError> {
            if self.cancel_during_call {
                request.cancellation.cancel();
            }
            self.requests.lock().expect("requests").push(request);
            let images = self
                .outcome
                .clone()
                .unwrap_or_else(|| Ok(vec![image(png(2, 3), Some("done"))]))?;
            Ok(ProviderImageOutput {
                images,
                usage: Some(InferenceUsage {
                    image_tokens: None,
                    audio_tokens: None,
                    total_tokens: None,
                    provider_reported_cost: None,
                    cache_write_tokens: None,
                    web_search_requests: None,
                    cached_input_tokens: None,
                    reasoning_tokens: None,
                    input_tokens: 12,
                    output_tokens: 4,
                }),
            })
        }
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(b"generated image bytes");
        bytes
    }

    fn image(bytes: Vec<u8>, text: Option<&str>) -> ProviderImage {
        ProviderImage {
            bytes,
            declared_mime_type: Some("image/png".into()),
            text: text.map(str::to_owned),
        }
    }

    struct Fixture {
        root: std::path::PathBuf,
        database: Database,
        media: LocalMediaBlobStore<Database, Database>,
        profile: ModelProfile,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).ok();
        }
    }

    fn fixture(kind: &str, protocol: ProviderProtocol, image_output: CapabilityStatus) -> Fixture {
        let root = std::env::temp_dir().join(format!("lettuce-image-{}", RequestId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let path = root.join("app.sqlite3");
        let database = Database::open(&path).expect("database");
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("authority");
        let media = LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write"),
            Database::open(&path).expect("blobs"),
            Database::open(&path).expect("assets"),
        );
        let account = ProviderAccountRepository::upsert(
            &database,
            ProviderAccount {
                id: ProviderAccountId::new(),
                secret_owner_id: SecretOwnerId::new(),
                provider_kind: kind.into(),
                protocol,
                label: "Images".into(),
                endpoint: Some("https://images.example".into()),
                enabled: true,
                streaming_enabled: true,
                allow_invalid_tls: false,
                api_key_ref: Some(SecretRef::new()),
                secret_headers: Vec::new(),
                config: ProviderConfig::Standard,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            None,
        )
        .expect("account");
        let profile = ModelProfileRepository::upsert(
            &database,
            ModelProfile {
                id: ModelProfileId::new(),
                provider_account_id: account.id,
                external_model_id: "image-model".into(),
                display_name: "Image model".into(),
                kind: ModelKind::Chat,
                config: ModelProfileConfig {
                    chat_parameters: Default::default(),
                    feature_parameters: Default::default(),
                    capabilities: ModelCapabilities {
                        input_modalities: ModalityCapabilities {
                            text: CapabilityStatus::Supported,
                            ..ModalityCapabilities::unknown()
                        },
                        output_modalities: ModalityCapabilities {
                            text: CapabilityStatus::Supported,
                            image: image_output,
                            ..ModalityCapabilities::unknown()
                        },
                        ..ModelCapabilities::default()
                    },
                    llama_cpp: Default::default(),
                    stable_diffusion: StableDiffusionSettings {
                        steps: Some(28),
                        extra_prompt: Some("high detail".into()),
                        base_loras: Some(vec![StableDiffusionLora {
                            path: "style.safetensors".into(),
                            multiplier: 0.8,
                            is_high_noise: false,
                            keywords: vec!["old trigger".into()],
                        }]),
                        ..StableDiffusionSettings::default()
                    },
                },
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(2),
                updated_at: TimestampMillis::new(2),
            },
            None,
        )
        .expect("profile");
        Fixture {
            root,
            database,
            media,
            profile,
        }
    }

    fn request(fixture: &Fixture) -> ImageGenerationRequest {
        ImageGenerationRequest {
            id: RequestId::new(),
            model_profile_id: fixture.profile.id,
            prompt: "a lighthouse".into(),
            settings: StableDiffusionSettings {
                steps: Some(8),
                ..StableDiffusionSettings::default()
            },
            input_images: Vec::new(),
            mask_image: None,
            loras: vec![StableDiffusionLora {
                path: "style.safetensors".into(),
                multiplier: 1.0,
                is_high_noise: false,
                keywords: vec!["NewTrigger".into()],
            }],
            size: Some("1024x1024".into()),
            quality: None,
            style: None,
            count: 1,
            source: ImageGenerationSource::Playground,
            attribution: ImageAttribution::default(),
            output_policy: ImageOutputPolicy::Retained,
            created_at: NOW,
        }
    }

    async fn run(
        fixture: &Fixture,
        request: ImageGenerationRequest,
        provider: &Provider,
    ) -> ImageGenerationRunResult {
        let coordinator = ImageGenerationCoordinator::new(&fixture.database, &fixture.database);
        let admitted = coordinator
            .admit(request, &fixture.database)
            .expect("admit");
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
        coordinator
            .run(
                work,
                &fixture.database,
                &fixture.media,
                provider,
                CancellationReason::User,
                TimestampMillis::new(2_000),
            )
            .await
            .expect("run")
    }

    #[tokio::test]
    async fn generations_compose_the_legacy_prompt_and_store_outputs_with_usage() {
        let fixture = fixture(
            "openai",
            ProviderProtocol::OpenAiCompatible,
            CapabilityStatus::Supported,
        );
        let input = fixture
            .media
            .ingest(
                png(4, 4).as_slice(),
                IngestRequest::new(
                    AssetKind::MessageImage,
                    AssetOrigin::Upload,
                    RetentionClass::Persistent,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("input image");
        let mut request = request(&fixture);
        request.input_images = vec![input.asset.id];
        let provider = Provider {
            outcome: Some(Ok(vec![
                image(png(2, 3), Some("done")),
                image(b"not an image".to_vec(), None),
            ])),
            ..Provider::default()
        };
        let result = run(&fixture, request.clone(), &provider).await;
        let ImageGenerationRunResult::Succeeded { record, job } = result else {
            panic!("expected success: {result:?}");
        };
        assert_eq!(job.state, JobState::Succeeded);
        let sent = provider.requests.lock().expect("requests");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].prompt, "high detail, NewTrigger, a lighthouse");
        assert_eq!(sent[0].settings.steps, Some(8));
        assert_eq!(sent[0].loras.len(), 1);
        assert_eq!(sent[0].loras[0].multiplier, 1.0);
        assert_eq!(sent[0].input_images.len(), 1);
        assert_eq!(sent[0].input_images[0].mime_type, "image/png");
        assert!(sent[0].text_output);
        assert_eq!(sent[0].external_model_id, "image-model");
        let ImageGenerationState::Succeeded { result } = &record.state else {
            panic!("expected result");
        };
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.rejected_outputs, 1);
        assert_eq!(result.images[0].text.as_deref(), Some("done"));
        assert_eq!(
            (result.images[0].width, result.images[0].height),
            (Some(2), Some(3))
        );
        let asset = MediaAssetRepository::get(&fixture.database, result.images[0].asset_id)
            .expect("asset read")
            .expect("asset");
        assert_eq!(asset.kind, AssetKind::GeneratedImage);
        assert_eq!(asset.provenance.producing_job_id, Some(job.id));
        let usage = fixture.database.job_usage(job.id).expect("usage");
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].model_profile_id, fixture.profile.id);
        assert!(matches!(
            &usage[0].result,
            Some(JobInferenceUsageResult::Response { usage: Some(usage), .. })
                if usage.input_tokens == 12
        ));
        let replay = ImageGenerationCoordinator::new(&fixture.database, &fixture.database)
            .admit(request, &fixture.database)
            .expect("replay admission");
        assert!(!replay.created);
        assert_eq!(replay.record, record);
        let history =
            lettuce_image_generation::PlaygroundHistoryRepository::list_playground_history(
                &fixture.database,
                30,
                None,
            )
            .expect("history");
        assert_eq!(history.len(), 1);
        assert_eq!(
            history[0].origin,
            lettuce_image_generation::PlaygroundOrigin::Generated
        );
        assert_eq!(history[0].job_id, Some(job.id));
        assert_eq!(history[0].status, "complete");
        assert_eq!(history[0].prompt, "a lighthouse");
        assert_eq!(history[0].model_profile_id, Some(fixture.profile.id));
        assert_eq!(
            history[0]
                .images
                .iter()
                .map(|image| image.asset_id)
                .collect::<Vec<_>>(),
            vec![Some(result.images[0].asset_id)]
        );
        let graph = crate::backup::backup_restore::assert_backup_round_trip(&fixture.database);
        assert_eq!(graph.job_backup.image_generations, vec![record.clone()]);
        assert_eq!(graph.playground_history.entries.len(), 1);
        assert_eq!(graph.playground_history.images.len(), 1);
        assert_eq!(
            lettuce_image_generation::PlaygroundHistoryRepository::delete_playground_history(
                &fixture.database,
                &history[0].id,
                true,
            ),
            Ok(vec![result.images[0].asset_id])
        );
        assert_eq!(
            MediaAssetRepository::get(&fixture.database, result.images[0].asset_id),
            Ok(None)
        );
        assert_eq!(
            lettuce_image_generation::PlaygroundHistoryRepository::delete_playground_history(
                &fixture.database,
                &history[0].id,
                true,
            ),
            Ok(Vec::new())
        );
        assert!(
            lettuce_image_generation::PlaygroundHistoryRepository::list_playground_history(
                &fixture.database,
                30,
                None,
            )
            .expect("history")
            .is_empty()
        );
    }

    #[tokio::test]
    async fn local_diffusion_requests_take_lora_keywords_from_the_library() {
        use lettuce_image_generation::sd_runtime::lora_library::{
            LoraArchitectureSource, LoraKeywordSource, LoraLibraryRepository, LoraRecord,
        };

        let fixture = fixture(
            "sdcpp",
            ProviderProtocol::StableDiffusion,
            CapabilityStatus::Supported,
        );
        fixture
            .database
            .save_lora(
                &LoraRecord {
                    path: "style.safetensors".into(),
                    filename: "style.safetensors".into(),
                    bytes_on_disk: 1,
                    modified_at: 1,
                    sha256: None,
                    keywords: vec!["LibraryTrigger".into()],
                    keyword_source: LoraKeywordSource::Manual,
                    architecture: None,
                    architecture_source: LoraArchitectureSource::None,
                },
                NOW,
            )
            .expect("library record");
        let provider = Provider::default();
        let request = request(&fixture);
        let request_id = request.id;
        let result = run(&fixture, request, &provider).await;
        assert!(matches!(result, ImageGenerationRunResult::Succeeded { .. }));
        let sent = provider.requests.lock().expect("requests");
        assert_eq!(sent[0].settings.seed, Some(super::playground_seed(request_id)));
        assert!(super::playground_seed(request_id) <= 2_147_483_647);
        assert_eq!(sent[0].prompt, "high detail, LibraryTrigger, a lighthouse");
        assert_eq!(sent[0].loras[0].keywords, vec!["LibraryTrigger"]);
    }

    #[tokio::test]
    async fn provider_failures_keep_the_legacy_message_and_record_failed_usage() {
        let fixture = fixture(
            "openai",
            ProviderProtocol::OpenAiCompatible,
            CapabilityStatus::Supported,
        );
        let provider = Provider {
            outcome: Some(Err(ImageProviderError::Failed(
                "API error 400: bad size".into(),
            ))),
            ..Provider::default()
        };
        let result = run(&fixture, request(&fixture), &provider).await;
        let ImageGenerationRunResult::Failed { record, job } = result else {
            panic!("expected failure: {result:?}");
        };
        assert_eq!(job.state, JobState::Failed);
        assert_eq!(
            record.state,
            ImageGenerationState::Failed {
                message: "API error 400: bad size".into(),
                completed_at: TimestampMillis::new(2_000),
            }
        );
        let usage = fixture.database.job_usage(job.id).expect("usage");
        assert_eq!(
            usage[0].result,
            Some(JobInferenceUsageResult::InferenceFailed)
        );
        let history =
            lettuce_image_generation::PlaygroundHistoryRepository::list_playground_history(
                &fixture.database,
                30,
                None,
            )
            .expect("history");
        assert_eq!(
            (history[0].status.as_str(), history[0].error.as_deref()),
            ("failed", Some("API error 400: bad size"))
        );
        assert!(history[0].images.is_empty());

        let provider = Provider {
            outcome: Some(Ok(vec![image(b"garbage".to_vec(), None)])),
            ..Provider::default()
        };
        let result = run(&fixture, request(&fixture), &provider).await;
        assert!(matches!(
            result,
            ImageGenerationRunResult::Failed { record, .. }
                if matches!(&record.state, ImageGenerationState::Failed { message, .. }
                    if message.starts_with("image output was rejected"))
        ));
    }

    #[tokio::test]
    async fn cancellation_during_the_call_discards_outputs() {
        let fixture = fixture(
            "openai",
            ProviderProtocol::OpenAiCompatible,
            CapabilityStatus::Supported,
        );
        let provider = Provider {
            cancel_during_call: true,
            ..Provider::default()
        };
        let result = run(&fixture, request(&fixture), &provider).await;
        let ImageGenerationRunResult::Cancelled { record, job } = result else {
            panic!("expected cancellation: {result:?}");
        };
        assert_eq!(job.state, JobState::Cancelled);
        assert_eq!(
            record.state,
            ImageGenerationState::Cancelled {
                completed_at: TimestampMillis::new(2_000)
            }
        );
    }

    #[test]
    fn interrupted_and_cancelled_jobs_settle_their_records_and_usage() {
        let fixture = fixture(
            "openai",
            ProviderProtocol::OpenAiCompatible,
            CapabilityStatus::Supported,
        );
        let coordinator = ImageGenerationCoordinator::new(&fixture.database, &fixture.database);
        let request = request(&fixture);
        let admitted = coordinator
            .admit(request.clone(), &fixture.database)
            .expect("admit");
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
        let usage_id = UsageEventId::new();
        fixture
            .database
            .admit_job_usage(JobInferenceUsage {
                id: usage_id,
                job_id: work.job.id,
                logical_attempt_id: GenerationAttemptId::new(),
                model_profile_id: fixture.profile.id,
                model_revision: fixture.profile.revision,
                provider_account_id: fixture.profile.provider_account_id,
                provider_account_revision: Revision::INITIAL,
                admitted_at: NOW,
                result: None,
            })
            .expect("usage");
        drop(work);
        fixture
            .database
            .expired_claims(
                TimestampMillis::new(TimestampMillis::now().expect("clock").get() + 3_600_000),
                10,
            )
            .expect("recover");
        let replay = coordinator
            .admit(request, &fixture.database)
            .expect("replay admission");
        assert!(!replay.created);
        assert_eq!(replay.job.state, JobState::Interrupted);
        assert!(matches!(
            &replay.record.state,
            ImageGenerationState::Failed { message, .. } if message == INTERRUPTED_MESSAGE
        ));
        assert_eq!(
            fixture.database.job_usage(replay.job.id).expect("usage")[0].result,
            Some(JobInferenceUsageResult::InferenceFailed)
        );

        let queued = coordinator
            .admit(super::tests::request(&fixture), &fixture.database)
            .expect("admit");
        fixture
            .database
            .append_and_transition(JobMutation::RequestCancellation {
                id: queued.job.id,
                reason: CancellationReason::User,
                at: queued.job.updated_at,
            })
            .expect("cancel queued job");
        let cancelled = JobStore::get(&fixture.database, queued.job.id)
            .expect("job")
            .expect("job");
        if !cancelled.state.is_terminal() {
            fixture
                .database
                .append_and_transition(JobMutation::FinishQueuedCancellation {
                    id: queued.job.id,
                    at: cancelled.updated_at,
                })
                .expect("finish queued cancellation");
        }
        assert!(
            coordinator
                .claim(
                    queued.job.id,
                    WorkerId::new(),
                    NOW,
                    Duration::from_secs(30),
                    &ResourceAvailability::all(),
                )
                .expect("claim")
                .is_none()
        );
        assert!(matches!(
            ImageGenerationRepository::get(&fixture.database, queued.job.id)
                .expect("record")
                .state,
            ImageGenerationState::Cancelled { .. }
        ));
    }

    #[test]
    fn models_without_image_output_are_refused_before_a_job_exists() {
        let fixture = fixture(
            "openai",
            ProviderProtocol::OpenAiCompatible,
            CapabilityStatus::Unknown,
        );
        let coordinator = ImageGenerationCoordinator::new(&fixture.database, &fixture.database);
        assert!(matches!(
            coordinator.admit(request(&fixture), &fixture.database),
            Err(ImageGenerationError::Profile(
                ImageProfileError::NotAnImageModel
            ))
        ));
        let missing = ImageGenerationRequest {
            model_profile_id: ModelProfileId::new(),
            ..request(&fixture)
        };
        assert!(matches!(
            coordinator.admit(missing, &fixture.database),
            Err(ImageGenerationError::ModelMissing)
        ));
    }
}
