//! The scene image a reply asks for, from the moment its follow-up exists to
//! the image on the message: starting one (automatically, when the user
//! approves, or when the user asks), settling it when the image job ends
//! (retrying a missing image up to three times) and recovering what a
//! restart interrupted.

use lettuce_characters::{CharacterRepository, PersonaRepository};
use lettuce_conversations::{
    ConversationRepository, SceneFollowUp, SceneFollowUpChange, SceneFollowUpMode,
    SceneFollowUpRepository, SceneFollowUpState,
};
use lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository;
use lettuce_image_generation::{
    ImageGenerationRecord, ImageGenerationRepository, ImageGenerationState, ImageMedia,
};
use lettuce_jobs::{JobKind, JobQuery, JobSnapshot, JobStore, SubjectId};
use lettuce_models::{ModelCatalog, ModelProfileRepository, ProviderAccountRepository};
use lettuce_types::{
    ConversationId, MessageId, PageLimit, PageRequest, RequestId, TimestampMillis,
};
use lettuce_usage::JobUsageLedger;

use crate::image::scene_image::{
    SceneImageError, SceneImageRequest, attach_scene_image, derived_id, scene_generation_request,
};
use crate::jobs::failure_labels as labels;
use crate::{ImageFeatureModelError, ImageGenerationCoordinator};

/// How many image jobs one follow-up may use when the provider returns no
/// image.
pub const MAX_ATTEMPTS: u32 = 3;

const NO_IMAGE_TEXT: &str = "no image";

/// The ports scene follow-ups read and write; the composition root's
/// database is one.
pub trait SceneFollowUpSources:
    ConversationRepository
    + SceneFollowUpRepository
    + CharacterRepository
    + PersonaRepository
    + ModelCatalog
    + ModelProfileRepository
    + ProviderAccountRepository
    + lettuce_settings::GlobalSettingsStore
    + lettuce_context::PromptRepository
    + ImageGenerationRepository
    + JobUsageLedger
    + LoraLibraryRepository
    + JobStore
{
}

impl<T> SceneFollowUpSources for T where
    T: ConversationRepository
        + SceneFollowUpRepository
        + CharacterRepository
        + PersonaRepository
        + ModelCatalog
        + ModelProfileRepository
        + ProviderAccountRepository
        + lettuce_settings::GlobalSettingsStore
        + lettuce_context::PromptRepository
        + ImageGenerationRepository
        + JobUsageLedger
        + LoraLibraryRepository
        + JobStore
{
}

/// Why a follow-up could not start.
#[derive(Debug, thiserror::Error)]
pub enum SceneFollowUpError {
    #[error("the message has no scene image follow-up")]
    NotFound,
    #[error("the scene image is already {0:?}")]
    WrongState(SceneFollowUpState),
    #[error(transparent)]
    Image(#[from] SceneImageError),
    #[error("scene follow-up storage failed")]
    Storage,
}

impl From<lettuce_conversations::SceneFollowUpError> for SceneFollowUpError {
    fn from(_: lettuce_conversations::SceneFollowUpError) -> Self {
        Self::Storage
    }
}

/// The label a follow-up stores for `error`.
pub fn failure_label(error: &SceneImageError) -> &'static str {
    match error {
        SceneImageError::Model(ImageFeatureModelError::SceneDisabled) => {
            labels::SCENE_IMAGE_DISABLED
        }
        SceneImageError::Model(
            ImageFeatureModelError::NoImageModel
            | ImageFeatureModelError::SceneModelUnsupported
            | ImageFeatureModelError::NoModel
            | ImageFeatureModelError::ModelNotFound,
        ) => labels::SCENE_IMAGE_NO_MODEL,
        SceneImageError::MessageUnavailable | SceneImageError::MessageNotFound => {
            labels::SCENE_IMAGE_MESSAGE_UNAVAILABLE
        }
        _ => labels::SCENE_IMAGE_FAILED,
    }
}

/// The root the request ids of one image ask derive from.
#[must_use]
pub fn root_request(message_id: MessageId, generation: u32) -> RequestId {
    RequestId::from_uuid(derived_id(
        RequestId::from_uuid(message_id.as_uuid()),
        &format!("scene-image-{generation}"),
    ))
}

#[must_use]
pub fn attempt_request(root: RequestId, attempt: u32) -> RequestId {
    RequestId::from_uuid(derived_id(root, &format!("attempt-{attempt}")))
}

#[derive(Debug)]
pub struct SceneFollowUps<'a, R: ?Sized, D: ?Sized> {
    repository: &'a R,
    media: &'a D,
}

impl<'a, R: ?Sized, D: ?Sized> SceneFollowUps<'a, R, D> {
    #[must_use]
    pub const fn new(repository: &'a R, media: &'a D) -> Self {
        Self { repository, media }
    }
}

impl<R, D> SceneFollowUps<'_, R, D>
where
    R: SceneFollowUpSources + ?Sized,
    D: ImageMedia + ?Sized,
{
    /// Starts the image of a follow-up in one of `from`: it becomes
    /// approved with `prompt` (the reply's own when none) and its first
    /// image job is queued. A follow-up in another state is `WrongState`;
    /// one whose job cannot be built or queued fails with the reason and the
    /// error is returned.
    pub fn start(
        &self,
        conversation_id: ConversationId,
        message_id: MessageId,
        from: &[SceneFollowUpState],
        prompt: Option<&str>,
        mode: Option<SceneFollowUpMode>,
        now: TimestampMillis,
    ) -> Result<(SceneFollowUp, JobSnapshot), SceneFollowUpError> {
        let current = self
            .repository
            .get_follow_up(conversation_id, message_id)?
            .ok_or(SceneFollowUpError::NotFound)?;
        let request_id = attempt_request(root_request(message_id, current.generation + 1), 1);
        let started = self
            .repository
            .change_follow_up(
                conversation_id,
                message_id,
                from,
                &SceneFollowUpChange {
                    state: Some(SceneFollowUpState::Approved),
                    prompt: prompt.map(str::to_owned),
                    mode,
                    request_id: Some(Some(request_id)),
                    attempt: Some(1),
                    next_generation: true,
                    failure: Some(None),
                },
                now,
            )?
            .ok_or_else(|| {
                SceneFollowUpError::WrongState(
                    self.repository
                        .get_follow_up(conversation_id, message_id)
                        .ok()
                        .flatten()
                        .map_or(current.state, |follow_up| follow_up.state),
                )
            })?;
        let job = self.admit_job(&started, now)?;
        Ok((started, job))
    }

    /// Queues the image job of the follow-up's current attempt, or returns
    /// the one already queued; a job that cannot be built fails the
    /// follow-up.
    pub fn admit_job(
        &self,
        follow_up: &SceneFollowUp,
        now: TimestampMillis,
    ) -> Result<JobSnapshot, SceneFollowUpError> {
        let Some(request_id) = follow_up.request_id else {
            return Err(SceneFollowUpError::Storage);
        };
        let built = scene_generation_request(
            self.repository,
            self.media,
            &SceneImageRequest {
                conversation_id: follow_up.conversation_id,
                message_id: follow_up.message_id,
                scene_prompt: follow_up.prompt.clone(),
                request_id,
            },
            now,
        )
        .map_err(SceneFollowUpError::Image)
        .and_then(|request| {
            ImageGenerationCoordinator::new(self.repository, self.repository)
                .admit(request, self.repository)
                .map(|admission| admission.job)
                .map_err(|error| {
                    SceneFollowUpError::Image(SceneImageError::Generation(error.to_string()))
                })
        });
        match built {
            Ok(job) => Ok(job),
            Err(error) => {
                let label = match &error {
                    SceneFollowUpError::Image(image) => failure_label(image),
                    _ => labels::SCENE_IMAGE_FAILED,
                };
                self.fail(follow_up, label, now)?;
                Err(error)
            }
        }
    }

    fn fail(
        &self,
        follow_up: &SceneFollowUp,
        label: &str,
        now: TimestampMillis,
    ) -> Result<Option<SceneFollowUp>, SceneFollowUpError> {
        Ok(self.repository.change_follow_up(
            follow_up.conversation_id,
            follow_up.message_id,
            &[
                SceneFollowUpState::Approved,
                SceneFollowUpState::Running,
                SceneFollowUpState::Pending,
            ],
            &SceneFollowUpChange {
                state: Some(SceneFollowUpState::Failed),
                failure: Some(Some(label.to_owned())),
                ..SceneFollowUpChange::default()
            },
            now,
        )?)
    }

    /// Settles the follow-up whose current attempt is `record`'s request,
    /// after its image job ended: the image is added to the message, a
    /// missing image gets another job (three in all), a user's cancellation
    /// dismisses it, and any other end fails it. `interrupted` says the app
    /// stopped during the job, so a cancellation is a failure the user can
    /// retry. A request no follow-up is generating for is left alone.
    pub fn settle(
        &self,
        record: &ImageGenerationRecord,
        interrupted: bool,
        now: TimestampMillis,
    ) -> Result<Option<SceneFollowUp>, SceneFollowUpError> {
        let Some(follow_up) = self.repository.follow_up_of_request(record.request.id)? else {
            return Ok(None);
        };
        if !follow_up.state.generating() {
            return Ok(None);
        }
        let root = root_request(follow_up.message_id, follow_up.generation);
        match &record.state {
            ImageGenerationState::Succeeded { result } => match result.images.first() {
                Some(image) => {
                    match attach_scene_image(
                        self.repository,
                        follow_up.conversation_id,
                        follow_up.message_id,
                        root,
                        image.asset_id,
                        now,
                    ) {
                        Ok(_) => self.done(&follow_up, now),
                        Err(error) => self.fail(&follow_up, failure_label(&error), now),
                    }
                }
                None => self.retry_or_fail(&follow_up, now),
            },
            ImageGenerationState::Failed { message, .. }
                if message.to_ascii_lowercase().contains(NO_IMAGE_TEXT) =>
            {
                self.retry_or_fail(&follow_up, now)
            }
            ImageGenerationState::Failed { .. } => {
                self.fail(&follow_up, labels::SCENE_IMAGE_FAILED, now)
            }
            ImageGenerationState::Cancelled { .. } if interrupted => {
                self.fail(&follow_up, labels::SCENE_IMAGE_INTERRUPTED, now)
            }
            ImageGenerationState::Cancelled { .. } => Ok(self.repository.change_follow_up(
                follow_up.conversation_id,
                follow_up.message_id,
                &[SceneFollowUpState::Approved, SceneFollowUpState::Running],
                &SceneFollowUpChange {
                    state: Some(SceneFollowUpState::Dismissed),
                    ..SceneFollowUpChange::default()
                },
                now,
            )?),
            ImageGenerationState::Pending => {
                self.fail(&follow_up, labels::SCENE_IMAGE_INTERRUPTED, now)
            }
        }
    }

    fn done(
        &self,
        follow_up: &SceneFollowUp,
        now: TimestampMillis,
    ) -> Result<Option<SceneFollowUp>, SceneFollowUpError> {
        Ok(self.repository.change_follow_up(
            follow_up.conversation_id,
            follow_up.message_id,
            &[SceneFollowUpState::Approved, SceneFollowUpState::Running],
            &SceneFollowUpChange {
                state: Some(SceneFollowUpState::Done),
                failure: Some(None),
                ..SceneFollowUpChange::default()
            },
            now,
        )?)
    }

    fn retry_or_fail(
        &self,
        follow_up: &SceneFollowUp,
        now: TimestampMillis,
    ) -> Result<Option<SceneFollowUp>, SceneFollowUpError> {
        if follow_up.attempt >= MAX_ATTEMPTS {
            return self.fail(follow_up, labels::SCENE_IMAGE_NO_IMAGE, now);
        }
        let attempt = follow_up.attempt + 1;
        let request_id = attempt_request(
            root_request(follow_up.message_id, follow_up.generation),
            attempt,
        );
        let Some(next) = self.repository.change_follow_up(
            follow_up.conversation_id,
            follow_up.message_id,
            &[SceneFollowUpState::Approved, SceneFollowUpState::Running],
            &SceneFollowUpChange {
                state: Some(SceneFollowUpState::Approved),
                request_id: Some(Some(request_id)),
                attempt: Some(attempt),
                ..SceneFollowUpChange::default()
            },
            now,
        )?
        else {
            return Ok(None);
        };
        self.admit_job(&next, now)?;
        Ok(Some(next))
    }

    /// Settles what a restart left unfinished: an image job that ended
    /// without its follow-up learning of it is settled from its record, a
    /// follow-up whose job was never queued gets one, one whose job is gone
    /// fails as interrupted, and an automatic follow-up the reply's turn
    /// never started starts now. Returns how many follow-ups changed.
    pub fn recover(&self, now: TimestampMillis) -> Result<usize, SceneFollowUpError> {
        let mut changed = 0;
        for follow_up in self
            .repository
            .follow_ups_in(&[SceneFollowUpState::Approved, SceneFollowUpState::Running])?
        {
            let Some(request_id) = follow_up.request_id else {
                self.fail(&follow_up, labels::SCENE_IMAGE_INTERRUPTED, now)?;
                changed += 1;
                continue;
            };
            match self.job_of_request(request_id)? {
                None if follow_up.state == SceneFollowUpState::Approved => {
                    if self.admit_job(&follow_up, now).is_ok() {
                        changed += 1;
                    }
                }
                None => {
                    self.fail(&follow_up, labels::SCENE_IMAGE_INTERRUPTED, now)?;
                    changed += 1;
                }
                Some(job) if job.state.is_terminal() => {
                    let record = ImageGenerationRepository::get(self.repository, job.id)
                        .map_err(|_| SceneFollowUpError::Storage)?;
                    self.settle(&record, true, now)?;
                    changed += 1;
                }
                Some(_) => {}
            }
        }
        for follow_up in self
            .repository
            .follow_ups_in(&[SceneFollowUpState::Pending])?
        {
            if follow_up.mode == SceneFollowUpMode::Auto
                && self
                    .start(
                        follow_up.conversation_id,
                        follow_up.message_id,
                        &[SceneFollowUpState::Pending],
                        None,
                        None,
                        now,
                    )
                    .is_ok()
            {
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// The image job of request `request_id`.
    pub fn job_of_request(
        &self,
        request_id: RequestId,
    ) -> Result<Option<JobSnapshot>, SceneFollowUpError> {
        job_of_scene_image_request(self.repository, request_id)
    }
}

/// The image job of request `request_id`.
pub fn job_of_scene_image_request<R: JobStore + ?Sized>(
    repository: &R,
    request_id: RequestId,
) -> Result<Option<JobSnapshot>, SceneFollowUpError> {
    let subject =
        SubjectId::new(request_id.to_string()).map_err(|_| SceneFollowUpError::Storage)?;
    Ok(JobStore::list(
        repository,
        JobQuery {
            state: None,
            kind: Some(JobKind::ImageGenerate),
            subject: Some(subject),
            page: PageRequest {
                cursor: None,
                limit: PageLimit::new(1),
            },
        },
    )
    .map_err(|_| SceneFollowUpError::Storage)?
    .items
    .into_iter()
    .next())
}
