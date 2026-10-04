//! Runs the queued image generation jobs. One handler claims every image
//! job (a scene image today), so a job is cancelled through `job_cancel` and
//! there is never a second claimer to race. A scene image's job settles its
//! follow-up when it ends.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError};
use lettuce_conversations::{SceneFollowUpChange, SceneFollowUpRepository, SceneFollowUpState};
use lettuce_image_generation::sd_runtime::output::{GenerationProgress, GenerationProgressSink};
use lettuce_image_generation::{ImageGenerationSource, ProgressHandle};
use lettuce_jobs::{
    CancellationReason, JobError, JobErrorCode, JobKind, JobMutation, JobSnapshot, JobStore,
    ResourceAvailability, ResourceClass, WorkerId, handle::CancellationToken,
};

use super::local::internal;
use super::runner::{ClaimedJob, JobHandler, JobLane, JobProgressSink};
use crate::api::ApiContext;
use crate::{
    ImageGenerationClaimedWork, ImageGenerationCoordinator, ImageGenerationRunResult,
    SceneFollowUps,
};

const IMAGE_LEASE: Duration = Duration::from_secs(60 * 60);
const LOCAL_LANE: &str = "image:local";
const MEDIA_UNAVAILABLE: &str = "image-media-unavailable";

/// The sink a local engine call reports its progress to for a job.
pub(super) fn progress_handle(progress: Arc<dyn JobProgressSink>) -> ProgressHandle {
    ProgressHandle(Arc::new(ProgressRelay(progress)))
}

/// Turns the local engine's progress into the job's typed image progress.
struct ProgressRelay(Arc<dyn JobProgressSink>);

impl GenerationProgressSink for ProgressRelay {
    fn progress(&self, progress: GenerationProgress) {
        self.0.image_progress(image_progress(progress));
    }
}

fn image_progress(progress: GenerationProgress) -> dto::ImageProgress {
    let (phase, step, total_steps, queue_position) = match progress {
        GenerationProgress::Starting => (dto::ImagePhase::Starting, None, None, None),
        GenerationProgress::Loading { step, steps } => {
            (dto::ImagePhase::Loading, Some(step), Some(steps), None)
        }
        GenerationProgress::Sampling { step, steps } => {
            (dto::ImagePhase::Sampling, Some(step), Some(steps), None)
        }
        GenerationProgress::Queued { queue_position } => {
            (dto::ImagePhase::Queued, None, None, queue_position)
        }
        GenerationProgress::Generating => (dto::ImagePhase::Generating, None, None, None),
        GenerationProgress::Retrying => (dto::ImagePhase::Retrying, None, None, None),
        GenerationProgress::Cancelled => (dto::ImagePhase::Cancelled, None, None, None),
    };
    dto::ImageProgress {
        phase,
        step,
        total_steps,
        queue_position,
        preview_asset: None,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ImageGenerateHandler;

#[async_trait]
impl JobHandler for ImageGenerateHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::ImageGenerate]
    }

    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        Some(JobLane(
            if job.resources.contains(&ResourceClass::Process) {
                LOCAL_LANE.to_owned()
            } else {
                format!("image:{}", job.id)
            },
        ))
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job_id = job.id;
        let local = job.resources.contains(&ResourceClass::Process);
        let snapshot = job.clone();
        let claimed = context
            .blocking(move |context| {
                let database = context.backend().database();
                if local && super::local::folder_move_active(context).is_err() {
                    cancel_queued_for_move(context, &snapshot, true)?;
                    return Ok(None);
                }
                let Some(work) = ImageGenerationCoordinator::new(database, database)
                    .claim(
                        job_id,
                        worker_id,
                        context.now(),
                        IMAGE_LEASE,
                        &ResourceAvailability::all(),
                    )
                    .map_err(internal)?
                else {
                    return Ok(None);
                };
                if work.record.request.source == ImageGenerationSource::Scene {
                    mark_running(database, work.record.request.id, context.now())
                        .map_err(internal)?;
                }
                Ok(Some(work))
            })
            .await?;
        Ok(claimed.map(|work| Box::new(ClaimedImage { work }) as Box<dyn ClaimedJob>))
    }
}

/// Cancels a queued local image job because the models folder is moving; no
/// engine call is made for it.
pub(super) fn cancel_queued_for_move(
    context: &ApiContext,
    job: &JobSnapshot,
    settle_generation: bool,
) -> Result<(), ApiError> {
    let database = context.backend().database();
    let at = context.now().max(job.updated_at);
    let requested = database
        .append_and_transition(JobMutation::RequestCancellation {
            id: job.id,
            reason: CancellationReason::Recovery,
            at,
        })
        .map_err(internal)?;
    database
        .append_and_transition(JobMutation::FinishQueuedCancellation {
            id: job.id,
            at: at.max(requested.updated_at),
        })
        .map_err(internal)?;
    if settle_generation {
        ImageGenerationCoordinator::new(database, database)
            .reconcile_after_restart(job.id)
            .map_err(internal)?;
    }
    Ok(())
}

struct ClaimedImage {
    work: ImageGenerationClaimedWork,
}

#[async_trait]
impl ClaimedJob for ClaimedImage {
    fn cancellation(&self) -> CancellationToken {
        self.work.handle.cancellation_token()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let work = self.work;
        let database = context.backend().database();
        let request_id = work.record.request.id;
        let source = work.record.request.source;
        let claim = work.claim.claim.clone();
        let lease_claim = claim.clone();
        let job_id = work.job.id;
        let Some(media) = context.media() else {
            let at = context.now().max(work.job.updated_at);
            database
                .append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: JobError::new(JobErrorCode::StorageFailure, false, MEDIA_UNAVAILABLE)
                        .expect("constant job error is valid"),
                    at,
                })
                .map_err(internal)?;
            lettuce_image_generation::ImageGenerationRepository::settle(
                database,
                job_id,
                lettuce_image_generation::ImageGenerationState::Failed {
                    message: MEDIA_UNAVAILABLE.to_owned(),
                    completed_at: at,
                },
            )
            .map_err(internal)?;
            fail_follow_up(
                &context,
                request_id,
                crate::jobs::failure_labels::SCENE_IMAGE_MEDIA_UNAVAILABLE,
            )?;
            return Ok(());
        };
        let shutting_down = context.shutdown_token().is_cancelled();
        let reason = if shutting_down {
            CancellationReason::Shutdown
        } else {
            CancellationReason::User
        };
        let now = context.now();
        let result = super::image_tools::renewing(
            &context,
            &lease_claim,
            IMAGE_LEASE,
            ImageGenerationCoordinator::new(database, database).run(
                work,
                database,
                media,
                context.image_provider(),
                crate::ImageRun {
                    progress: Some(progress_handle(progress)),
                    cancellation_reason: reason,
                    now,
                },
            ),
        )
        .await;
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                fail_follow_up(
                    &context,
                    request_id,
                    crate::jobs::failure_labels::SCENE_IMAGE_FAILED,
                )?;
                if let Some(job) = database.get(job_id).map_err(internal)?
                    && !job.is_terminal()
                {
                    database
                        .append_and_transition(JobMutation::Fail {
                            claim,
                            error: JobError::new(
                                JobErrorCode::StorageFailure,
                                false,
                                crate::jobs::failure_labels::SCENE_IMAGE_FAILED,
                            )
                            .expect("constant label"),
                            at: context.now().max(job.updated_at),
                        })
                        .map_err(internal)?;
                }
                if source == ImageGenerationSource::Scene {
                    lettuce_image_generation::ImageGenerationRepository::settle(
                        database,
                        job_id,
                        lettuce_image_generation::ImageGenerationState::Failed {
                            message: crate::jobs::failure_labels::SCENE_IMAGE_FAILED.to_owned(),
                            completed_at: context.now(),
                        },
                    )
                    .map_err(internal)?;
                }
                return Err(internal(error));
            }
        };
        let (ImageGenerationRunResult::Succeeded { record, .. }
        | ImageGenerationRunResult::Failed { record, .. }
        | ImageGenerationRunResult::Cancelled { record, .. }) = result;
        if record.request.source == ImageGenerationSource::Scene {
            SceneFollowUps::new(database, media)
                .settle(
                    &record,
                    context.shutdown_token().is_cancelled(),
                    context.now(),
                )
                .map_err(internal)?;
        }
        Ok(())
    }
}

/// Moves the scene image follow-up whose job starts from approved to running.
fn mark_running<R: SceneFollowUpRepository + ?Sized>(
    repository: &R,
    request_id: lettuce_types::RequestId,
    now: lettuce_types::TimestampMillis,
) -> Result<(), lettuce_conversations::SceneFollowUpError> {
    if let Some(follow_up) = repository.follow_up_of_request(request_id)? {
        repository.change_follow_up(
            follow_up.conversation_id,
            follow_up.target,
            &[SceneFollowUpState::Approved],
            &SceneFollowUpChange {
                state: Some(SceneFollowUpState::Running),
                ..SceneFollowUpChange::default()
            },
            now,
        )?;
    }
    Ok(())
}

fn fail_follow_up(
    context: &ApiContext,
    request_id: lettuce_types::RequestId,
    label: &str,
) -> Result<(), ApiError> {
    let database = context.backend().database();
    if let Some(follow_up) = database
        .follow_up_of_request(request_id)
        .map_err(internal)?
    {
        crate::api::scenes::fail_follow_up(database, &follow_up, label, context.now())?;
    }
    Ok(())
}
