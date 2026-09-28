//! Runs the queued image generation jobs. One handler claims every image
//! job (a scene image today), so a job is cancelled through `job_cancel` and
//! there is never a second claimer to race. A scene image's job settles its
//! follow-up when it ends.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lettuce_contracts::ApiError;
use lettuce_conversations::{SceneFollowUpChange, SceneFollowUpRepository, SceneFollowUpState};
use lettuce_image_generation::ImageGenerationSource;
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
        let claimed = context
            .blocking(move |context| {
                let database = context.backend().database();
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
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let work = self.work;
        let database = context.backend().database();
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
            return Ok(());
        };
        let shutting_down = context.shutdown_token().is_cancelled();
        let reason = if shutting_down {
            CancellationReason::Shutdown
        } else {
            CancellationReason::User
        };
        let now = context.now();
        let result = ImageGenerationCoordinator::new(database, database)
            .run(work, database, media, context.image_provider(), reason, now)
            .await
            .map_err(internal)?;
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
            follow_up.message_id,
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
