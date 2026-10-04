//! The image jobs besides generation: upscaling a stored image, the local
//! engine's runnability probes and LoRA keyword discovery. Admitting one
//! records what it works on next to the job; the runner claims it, runs it
//! with the job's own cancellation and stores what it found before the job
//! settles, so the job's result carries it. Work on the local engine shares
//! one lane with image generation.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_image_generation::sd_runtime::runnability::{
    CatalogRunnabilityRequest, RemoteBundleRunnabilityRequest, Runnability, RunnabilityStatus,
};
use lettuce_image_generation::{ImageError, ImageFailureKind};
use lettuce_jobs::{
    CancellationPolicy, ClaimRef, IdempotencyKey, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobPriority, JobSnapshot, JobSpec, JobState, JobStore, JobSubject, OutcomeRef,
    RecoveryPolicy, ResourceAvailability, ResourceClass, StageSnapshot, SubjectKind, WorkerId,
    handle::CancellationToken,
};
use lettuce_types::{AssetId, JobId, RequestId};
use serde::{Deserialize, Serialize};

use super::image::progress_handle;
use super::local::{digest, operation_key, stable_uuid};
use super::runner::{ClaimedJob, JobHandler, JobLane, JobProgressSink};
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, parse_id};
use crate::api::image::{engine, failure_kind, internal, lora_library};

const TOOL_LEASE: Duration = Duration::from_secs(30 * 60);
pub(super) const LOCAL_LANE: &str = "image:local";
const LORA_LANE: &str = "image:lora";
const UPSCALE_PREFIX: &str = "image-upscale-";
const RUNNABILITY_PREFIX: &str = "sd-runnability-";
const BUNDLE_RUNNABILITY_PREFIX: &str = "sd-bundle-runnability-";
const LORA_PREFIX: &str = "lora-discover-";

/// What an image tool job works on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ImageToolDetail {
    Upscale {
        asset_id: String,
        entry_id: Option<String>,
    },
    Runnability {
        request: dto::SdRunnabilityRequest,
    },
    BundleRunnability {
        request: dto::SdBundleRunnabilityRequest,
    },
    LoraDiscover {
        path: String,
        profile_id: Option<String>,
    },
}

impl ImageToolDetail {
    const fn prefix(&self) -> &'static str {
        match self {
            Self::Upscale { .. } => UPSCALE_PREFIX,
            Self::Runnability { .. } => RUNNABILITY_PREFIX,
            Self::BundleRunnability { .. } => BUNDLE_RUNNABILITY_PREFIX,
            Self::LoraDiscover { .. } => LORA_PREFIX,
        }
    }

    const fn job_kind(&self) -> JobKind {
        match self {
            Self::Upscale { .. } => JobKind::MediaTransform,
            Self::Runnability { .. } | Self::BundleRunnability { .. } => JobKind::RuntimePrepare,
            Self::LoraDiscover { .. } => JobKind::Maintenance,
        }
    }

    const fn stage(&self) -> &'static str {
        match self {
            Self::Upscale { .. } => "upscale",
            Self::Runnability { .. } | Self::BundleRunnability { .. } => "runnability",
            Self::LoraDiscover { .. } => "lora-discovery",
        }
    }

    fn resources(&self) -> Vec<ResourceClass> {
        match self {
            Self::Upscale { .. } | Self::Runnability { .. } => vec![
                ResourceClass::ModelLoad,
                ResourceClass::Gpu,
                ResourceClass::Process,
                ResourceClass::DiskWrite,
            ],
            Self::BundleRunnability { .. } => vec![ResourceClass::Process],
            Self::LoraDiscover { .. } => vec![ResourceClass::DiskRead, ResourceClass::Network],
        }
    }
}

/// What an image tool job found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ImageToolResult {
    Upscaled {
        upscaled: dto::ImageUpscaled,
    },
    Runnability {
        verdict: dto::SdRunnability,
    },
    LoraDiscovered {
        discovery: dto::LoraKeywordDiscovery,
    },
}

/// The job an earlier request with this key started; a different request
/// under the same key is a conflict.
pub(crate) fn replay_operation(
    context: &ApiContext,
    key: &str,
    request_digest: &str,
) -> Result<Option<JobId>, ApiError> {
    match context
        .backend()
        .database()
        .job_operation(key)
        .map_err(internal)?
    {
        Some(prior) if prior.request_digest == request_digest => Ok(Some(prior.job_id)),
        Some(_) => Err(api_error(
            ApiErrorCode::Conflict,
            "client_operation_id was already used for a different request",
        )),
        None => Ok(None),
    }
}

/// Queues an image tool job under the client's operation id and returns it;
/// repeating the request returns the same job.
pub(crate) fn admit_tool(
    context: &ApiContext,
    command: &str,
    client_operation_id: &str,
    detail: ImageToolDetail,
    precheck: impl FnOnce() -> Result<(), ApiError>,
) -> Result<dto::JobAccepted, ApiError> {
    let key = operation_key(command, client_operation_id)?;
    let request_digest = digest(&detail)?;
    if let Some(job_id) = replay_operation(context, &key, &request_digest)? {
        return Ok(dto::JobAccepted {
            job_id: job_id.to_string(),
        });
    }
    precheck()?;
    let uuid = stable_uuid(&[command, &key]);
    let spec = JobSpec::new(
        detail.job_kind(),
        JobSubject::from_uuid(SubjectKind::ImageRequest, uuid),
        OutcomeRef::Request(RequestId::from_uuid(uuid)),
    )
    .with_idempotency_key(
        IdempotencyKey::new(format!("{}{uuid}", detail.prefix())).map_err(internal)?,
    )
    .with_priority(JobPriority::Interactive)
    .with_resources(detail.resources())
    .with_policies(
        RecoveryPolicy::MarkInterrupted,
        CancellationPolicy::Cooperative,
    );
    let job = context
        .backend()
        .database()
        .admit_job_with_detail(
            spec,
            &key,
            &request_digest,
            &serde_json::to_value(&detail).map_err(internal)?,
        )
        .map_err(|error| match error {
            lettuce_jobs::StoreError::IdempotencyConflict => api_error(
                ApiErrorCode::Conflict,
                "client_operation_id was already used for a different request",
            ),
            error => internal(error),
        })?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job.id.to_string(),
    })
}

/// Whether `job` is a LoRA keyword discovery.
pub(crate) fn is_lora_discovery(job: &JobSnapshot) -> bool {
    job.idempotency_key
        .as_ref()
        .is_some_and(|key| key.as_str().starts_with(LORA_PREFIX))
}

fn is_tool(job: &JobSnapshot) -> bool {
    job.idempotency_key.as_ref().is_some_and(|key| {
        [
            UPSCALE_PREFIX,
            RUNNABILITY_PREFIX,
            BUNDLE_RUNNABILITY_PREFIX,
            LORA_PREFIX,
        ]
        .iter()
        .any(|prefix| key.as_str().starts_with(prefix))
    })
}

/// What the job view shows of a tool job: its result and why it failed.
pub(crate) fn tool_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<(Option<dto::JobResultDto>, Option<dto::ImageFailure>), ApiError> {
    if !is_tool(job) {
        return Ok((None, None));
    }
    let Some(record) = context
        .backend()
        .database()
        .job_detail(job.id)
        .map_err(internal)?
    else {
        return Ok((None, None));
    };
    let result = record
        .result
        .and_then(|result| serde_json::from_value::<ImageToolResult>(result).ok())
        .map(|result| match result {
            ImageToolResult::Upscaled { upscaled } => dto::JobResultDto::ImageUpscaled { upscaled },
            ImageToolResult::Runnability { verdict } => dto::JobResultDto::Runnability {
                verdict: Box::new(verdict),
            },
            ImageToolResult::LoraDiscovered { discovery } => dto::JobResultDto::LoraDiscovered {
                discovered: dto::LoraDiscovered { discovery },
            },
        });
    let failure = record
        .failure
        .and_then(|failure| serde_json::from_value::<dto::ImageFailure>(failure).ok());
    Ok((result, failure))
}

/// Runs the queued image tool jobs the API admitted. A queued job of a tool
/// kind without a tool key belongs to a caller that runs it itself and is
/// left alone.
#[derive(Debug, Clone, Copy, Default)]
pub struct ImageToolHandler;

#[async_trait]
impl JobHandler for ImageToolHandler {
    fn kinds(&self) -> &[JobKind] {
        &[
            JobKind::MediaTransform,
            JobKind::RuntimePrepare,
            JobKind::Maintenance,
        ]
    }

    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        if !is_tool(job) {
            return None;
        }
        let lora = job
            .idempotency_key
            .as_ref()
            .is_some_and(|key| key.as_str().starts_with(LORA_PREFIX));
        Some(JobLane(
            if lora { LORA_LANE } else { LOCAL_LANE }.to_owned(),
        ))
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job_id = job.id;
        let claimed = context
            .blocking(move |context| claim_tool(context, job_id, worker_id))
            .await?;
        Ok(claimed.map(|claimed| Box::new(claimed) as Box<dyn ClaimedJob>))
    }
}

struct ClaimedTool {
    job_id: JobId,
    claim: ClaimRef,
    detail: ImageToolDetail,
    cancellation: CancellationToken,
}

fn claim_tool(
    context: &ApiContext,
    job_id: JobId,
    worker_id: WorkerId,
) -> Result<Option<ClaimedTool>, ApiError> {
    let database = context.backend().database();
    let Some(job) = database.get(job_id).map_err(IntoApiError::into_api_error)? else {
        return Ok(None);
    };
    let detail = database
        .job_detail(job_id)
        .map_err(internal)?
        .and_then(|record| serde_json::from_value::<ImageToolDetail>(record.detail).ok());
    let at = context.now().max(job.updated_at);
    let Some(claim) = database
        .claim(
            job_id,
            worker_id,
            at,
            TOOL_LEASE,
            &ResourceAvailability::all(),
        )
        .map_err(IntoApiError::into_api_error)?
    else {
        return Ok(None);
    };
    let started = database.append_and_transition(JobMutation::Start {
        claim: claim.claim.clone(),
        at,
    });
    if started.is_err() {
        let current = database.get(job_id).map_err(IntoApiError::into_api_error)?;
        if current.is_some_and(|job| job.state == JobState::CancellationRequested) {
            crate::models::artifact_install::finish_claimed_cancellation(
                database,
                &claim.claim,
                at,
            )
            .map_err(IntoApiError::into_api_error)?;
            return Ok(None);
        }
        started.map_err(IntoApiError::into_api_error)?;
    }
    let Some(detail) = detail else {
        database
            .append_and_transition(JobMutation::Fail {
                claim: claim.claim,
                error: tool_error(ImageFailureKind::InvalidRequest, false),
                at,
            })
            .map_err(IntoApiError::into_api_error)?;
        return Ok(None);
    };
    database
        .append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new(detail.stage(), false).map_err(internal)?,
            at,
        })
        .map_err(IntoApiError::into_api_error)?;
    Ok(Some(ClaimedTool {
        job_id,
        claim: claim.claim,
        detail,
        cancellation: CancellationToken::new(),
    }))
}

fn tool_error(kind: ImageFailureKind, retryable: bool) -> JobError {
    let code = match kind {
        ImageFailureKind::InvalidRequest | ImageFailureKind::LoraInvalid => {
            JobErrorCode::InvalidInput
        }
        ImageFailureKind::StorageFailed => JobErrorCode::StorageFailure,
        ImageFailureKind::RuntimeNotInstalled
        | ImageFailureKind::UpscalerMissing
        | ImageFailureKind::ModelFileMissing
        | ImageFailureKind::ModelNotConfigured
        | ImageFailureKind::LocalUnsupported => JobErrorCode::CapabilityUnavailable,
        ImageFailureKind::EngineTimedOut => JobErrorCode::TimedOut,
        _ => JobErrorCode::WorkerFailed,
    };
    JobError::new(code, retryable, kind.label()).expect("image failure labels are valid")
}

/// Keeps the job's claim alive for as long as `work` runs.
pub(super) async fn renewing<T>(
    context: &ApiContext,
    claim: &ClaimRef,
    lease: Duration,
    work: impl Future<Output = T>,
) -> T {
    tokio::pin!(work);
    loop {
        tokio::select! {
            output = &mut work => return output,
            () = tokio::time::sleep(lease / 3) => {
                let owned = claim.clone();
                let renewed = context
                    .blocking(move |context| {
                        context
                            .backend()
                            .database()
                            .heartbeat(&owned, context.now(), lease)
                            .map_err(IntoApiError::into_api_error)
                    })
                    .await;
                if let Err(error) = renewed {
                    tracing::warn!(job_id = %claim.job_id, message = %error.message, "could not renew the image job lease");
                }
            }
        }
    }
}

enum ToolOutcome {
    Succeeded(ImageToolResult),
    Failed(ImageError),
    Cancelled,
}

#[async_trait]
impl ClaimedJob for ClaimedTool {
    fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let claim = self.claim.clone();
        let outcome = renewing(
            &context,
            &claim,
            TOOL_LEASE,
            self.execute(&context, progress),
        )
        .await;
        let job_id = self.job_id;
        context
            .blocking(move |context| settle_tool(context, job_id, claim, outcome))
            .await
    }
}

impl ClaimedTool {
    async fn execute(
        &self,
        context: &ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> ToolOutcome {
        let result = match &self.detail {
            ImageToolDetail::Upscale { asset_id, entry_id } => {
                self.upscale(context, asset_id, entry_id.clone()).await
            }
            ImageToolDetail::Runnability { request } => {
                self.probe(context, request, progress).await
            }
            ImageToolDetail::BundleRunnability { request } => {
                self.bundle_estimate(context, request).await
            }
            ImageToolDetail::LoraDiscover { path, profile_id } => {
                self.discover(context, path, profile_id.clone()).await
            }
        };
        match result {
            Ok(result) => ToolOutcome::Succeeded(result),
            Err(error) if error.is_cancelled() => ToolOutcome::Cancelled,
            Err(_) if context.shutdown_token().is_cancelled() => ToolOutcome::Cancelled,
            Err(error) => ToolOutcome::Failed(error),
        }
    }

    async fn upscale(
        &self,
        context: &ApiContext,
        asset_id: &str,
        entry_id: Option<String>,
    ) -> Result<ImageToolResult, ImageError> {
        let asset: AssetId = parse_id(asset_id, "asset_id")
            .map_err(|error| ImageError::new(ImageFailureKind::InvalidRequest, error.message))?;
        let Some(media) = context.media() else {
            return Err(ImageError::storage("the media store is unavailable"));
        };
        let image = self.upscaled(context, media, asset).await?;
        let history_id = match entry_id {
            Some(entry_id) => {
                let (context, job_id, recorded) = (context.clone(), self.job_id, image.clone());
                Some(
                    context
                        .blocking(move |context| {
                            crate::api::image::record_upscale(context, job_id, entry_id, recorded)
                        })
                        .await
                        .map_err(|error| ImageError::storage(error.message))?,
                )
            }
            None => None,
        };
        Ok(ImageToolResult::Upscaled {
            upscaled: dto::ImageUpscaled {
                asset: context.asset_ref(image.asset_id),
                mime_type: image.mime_type,
                width: image.width,
                height: image.height,
                history_id,
            },
        })
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    async fn upscaled(
        &self,
        context: &ApiContext,
        media: &crate::api::ApiMediaStore,
        asset: AssetId,
    ) -> Result<lettuce_image_generation::GeneratedImage, ImageError> {
        context
            .backend()
            .upscale_image(media, asset, &self.cancellation)
            .await
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    async fn upscaled(
        &self,
        _context: &ApiContext,
        _media: &crate::api::ApiMediaStore,
        _asset: AssetId,
    ) -> Result<lettuce_image_generation::GeneratedImage, ImageError> {
        Err(ImageError::new(
            ImageFailureKind::LocalUnsupported,
            "Local stable-diffusion.cpp image generation is desktop-only.",
        ))
    }

    async fn probe(
        &self,
        context: &ApiContext,
        request: &dto::SdRunnabilityRequest,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<ImageToolResult, ImageError> {
        let engine = engine(context).map_err(api_image_error)?;
        let verdict = engine
            .catalog_runnability(
                catalog_request(request),
                &self.cancellation,
                Some(progress_handle(progress).0),
            )
            .await?;
        Ok(ImageToolResult::Runnability {
            verdict: verdict_dto(verdict),
        })
    }

    async fn bundle_estimate(
        &self,
        context: &ApiContext,
        request: &dto::SdBundleRunnabilityRequest,
    ) -> Result<ImageToolResult, ImageError> {
        let engine = engine(context).map_err(api_image_error)?;
        let bundle = RemoteBundleRunnabilityRequest {
            profile_id: request.profile_id.clone(),
            runtime_release: request.runtime_release.clone(),
            runtime_asset: request.runtime_asset.clone(),
            diffusion_bytes: request.diffusion_bytes,
            text_encoder_bytes: request.text_encoder_bytes,
            vae_bytes: request.vae_bytes,
            vision_encoder_bytes: request.vision_encoder_bytes,
        };
        let estimate = engine.remote_bundle_runnability(&bundle);
        let verdict = tokio::select! {
            biased;
            () = self.cancellation.cancelled() => return Err(ImageError::cancelled()),
            verdict = estimate => verdict?,
        };
        Ok(ImageToolResult::Runnability {
            verdict: verdict_dto(verdict),
        })
    }

    async fn discover(
        &self,
        context: &ApiContext,
        path: &str,
        profile_id: Option<String>,
    ) -> Result<ImageToolResult, ImageError> {
        let client = context
            .backend()
            .tls_policy()
            .map_err(|error| ImageError::storage(error.to_string()))
            .and_then(|tls| {
                lettuce_network::BulkHttpClient::with_tls(&tls)
                    .map_err(|error| ImageError::storage(error.to_string()))
            })?;
        let (context, path, token) = (context.clone(), path.to_owned(), self.cancellation.clone());
        let library_context = context.clone();
        let discovery = async {
            let library = lora_library(&library_context).map_err(|error| {
                ImageError::new(ImageFailureKind::LocalUnsupported, error.message)
            })?;
            library
                .discover(&path, profile_id.as_deref(), &client, context.now(), &token)
                .await
        }
        .await?;
        Ok(ImageToolResult::LoraDiscovered {
            discovery: crate::api::image::lora_discovery(discovery),
        })
    }
}

fn api_image_error(error: ApiError) -> ImageError {
    ImageError::new(ImageFailureKind::LocalUnsupported, error.message)
}

fn catalog_request(request: &dto::SdRunnabilityRequest) -> CatalogRunnabilityRequest {
    CatalogRunnabilityRequest {
        profile_id: request.profile_id.clone(),
        variant_id: request.variant_id.clone(),
        runtime_release: request.runtime_release.clone(),
        runtime_asset: request.runtime_asset.clone(),
        width: request.width,
        height: request.height,
        reference_image_count: request.reference_image_count,
        reference_images: Vec::new(),
        loras: request
            .loras
            .iter()
            .map(crate::api::image::to_engine_lora)
            .collect(),
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone(),
        sample_steps: request.sample_steps,
        cfg_scale: request.cfg_scale,
        seed: request.seed,
        sample_method: request.sample_method.clone(),
        batch_count: request.batch_count,
        full_execution: request.full_execution,
    }
}

fn verdict_dto(verdict: Runnability) -> dto::SdRunnability {
    dto::SdRunnability {
        status: match verdict.status {
            RunnabilityStatus::IncompatibleRuntime => dto::RunnabilityStatus::IncompatibleRuntime,
            RunnabilityStatus::NotInstalled => dto::RunnabilityStatus::NotInstalled,
            RunnabilityStatus::EstimatedRunnable => dto::RunnabilityStatus::EstimatedRunnable,
            RunnabilityStatus::CpuFallback => dto::RunnabilityStatus::CpuFallback,
            RunnabilityStatus::Inconclusive => dto::RunnabilityStatus::Inconclusive,
            RunnabilityStatus::Passed => dto::RunnabilityStatus::Passed,
            RunnabilityStatus::Failed => dto::RunnabilityStatus::Failed,
        },
        method: verdict.method.to_owned(),
        exact: verdict.exact,
        scope: verdict.scope.to_owned(),
        placement_policy: verdict.placement_policy.to_owned(),
        elapsed_ms: verdict.elapsed_ms,
        reason: verdict.reason,
        estimate: verdict.estimate.map(crate::api::image::fit_estimate),
    }
}

/// Ends a started tool job: success records what it produced, a failure
/// records why, a cancellation (by the user or by shutdown) is cleaned up.
fn settle_tool(
    context: &ApiContext,
    job_id: JobId,
    claim: ClaimRef,
    outcome: ToolOutcome,
) -> Result<(), ApiError> {
    let database = context.backend().database();
    let job = database
        .get(job_id)
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "job was not found"))?;
    if job.state.is_terminal() {
        return Ok(());
    }
    let at = context.now().max(job.updated_at);
    match outcome {
        ToolOutcome::Succeeded(result) => {
            let recorded = serde_json::to_value(&result)
                .map_err(internal)
                .and_then(|value| {
                    database
                        .record_job_detail_result(job_id, &value)
                        .map_err(internal)
                });
            if !matches!(recorded, Ok(true)) {
                database
                    .append_and_transition(JobMutation::Fail {
                        claim,
                        error: tool_error(ImageFailureKind::StorageFailed, true),
                        at,
                    })
                    .map_err(IntoApiError::into_api_error)?;
                return Ok(());
            }
            database
                .append_and_transition(JobMutation::Succeed {
                    claim,
                    outcome: JobOutcome::Success {
                        result_ref: OutcomeRef::Request(RequestId::from_uuid(stable_uuid(&[
                            "image-tool",
                            &job_id.to_string(),
                        ]))),
                    },
                    at,
                })
                .map_err(IntoApiError::into_api_error)?;
        }
        ToolOutcome::Failed(error) => {
            let failure = dto::ImageFailure {
                kind: failure_kind(error.kind),
                message: error.message.clone(),
            };
            if let Ok(value) = serde_json::to_value(&failure)
                && let Err(error) = database.record_job_detail_failure(job_id, &value)
            {
                tracing::warn!(%job_id, %error, "an image job's failure could not be recorded");
            }
            database
                .append_and_transition(JobMutation::Fail {
                    claim,
                    error: tool_error(error.kind, false),
                    at,
                })
                .map_err(IntoApiError::into_api_error)?;
        }
        ToolOutcome::Cancelled => {
            if job.state != JobState::CancellationRequested {
                database
                    .append_and_transition(JobMutation::RequestCancellation {
                        id: job_id,
                        reason: lettuce_jobs::CancellationReason::Shutdown,
                        at,
                    })
                    .map_err(IntoApiError::into_api_error)?;
            }
            crate::models::artifact_install::finish_claimed_cancellation(database, &claim, at)
                .map_err(IntoApiError::into_api_error)?;
        }
    }
    Ok(())
}
