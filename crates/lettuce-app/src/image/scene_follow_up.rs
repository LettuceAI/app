//! The scene image a reply asks for, from the moment its follow-up exists to
//! the image on the message: starting one (automatically, when the user
//! approves, or when the user asks), settling it when the image job ends
//! (retrying a missing image up to three times) and recovering what a
//! restart interrupted.

use lettuce_characters::{CharacterRepository, PersonaRepository};
use lettuce_conversations::{
    ConversationRepository, SceneFollowUp, SceneFollowUpChange, SceneFollowUpMode,
    SceneFollowUpRepository, SceneFollowUpState, SceneFollowUpTarget,
};
use lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository;
use lettuce_image_generation::{
    ImageGenerationRecord, ImageGenerationRepository, ImageGenerationState, ImageMedia,
};
use lettuce_jobs::{JobKind, JobQuery, JobSnapshot, JobStore, SubjectId};
use lettuce_models::{ModelCatalog, ModelProfileRepository, ProviderAccountRepository};
use lettuce_types::{ConversationId, PageLimit, PageRequest, RequestId, TimestampMillis};
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
    + lettuce_conversations::ConversationOverviewReader
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
        + lettuce_conversations::ConversationOverviewReader
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
pub fn root_request(target: SceneFollowUpTarget, generation: u32) -> RequestId {
    RequestId::from_uuid(derived_id(
        RequestId::from_uuid(match target {
            SceneFollowUpTarget::Candidate(id) => id.as_uuid(),
            SceneFollowUpTarget::StarterRevision(id) => id.as_uuid(),
        }),
        &format!("scene-image-{}-{generation}", target.kind()),
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
        target: SceneFollowUpTarget,
        from: &[SceneFollowUpState],
        prompt: Option<&str>,
        mode: Option<SceneFollowUpMode>,
        now: TimestampMillis,
    ) -> Result<(SceneFollowUp, JobSnapshot), SceneFollowUpError> {
        let current = self
            .repository
            .get_follow_up(conversation_id, target)?
            .ok_or(SceneFollowUpError::NotFound)?;
        let request_id = attempt_request(root_request(target, current.generation + 1), 1);
        let started = self.repository.change_follow_up(
            conversation_id,
            target,
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
        )?;
        let Some(started) = started else {
            let state = self
                .repository
                .get_follow_up(conversation_id, target)?
                .ok_or(SceneFollowUpError::NotFound)?
                .state;
            return Err(SceneFollowUpError::WrongState(state));
        };
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
                .map_err(|error| match error {
                    crate::ImageGenerationError::Models(_)
                    | crate::ImageGenerationError::Repository(_)
                    | crate::ImageGenerationError::Jobs(_)
                    | crate::ImageGenerationError::Usage
                    | crate::ImageGenerationError::LoraLibrary(_) => SceneFollowUpError::Storage,
                    _ => SceneFollowUpError::Image(SceneImageError::Generation(error.to_string())),
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
            follow_up.target,
            &[
                SceneFollowUpState::Approved,
                SceneFollowUpState::Running,
                SceneFollowUpState::AwaitingTurn,
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
        if follow_up.state == SceneFollowUpState::AwaitingTurn {
            complete_one(self.repository, &follow_up, now)?;
            return Ok(self
                .repository
                .get_follow_up(follow_up.conversation_id, follow_up.target)?);
        }
        let root = root_request(follow_up.target, follow_up.generation);
        match &record.state {
            ImageGenerationState::Succeeded { result } => match result.images.first() {
                Some(image) => {
                    match attach_scene_image(
                        self.repository,
                        follow_up.conversation_id,
                        follow_up.message_id,
                        follow_up.target,
                        root,
                        image.asset_id,
                        now,
                    ) {
                        Ok(_) => Ok(self
                            .repository
                            .get_follow_up(follow_up.conversation_id, follow_up.target)?),
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
                follow_up.target,
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
            root_request(follow_up.target, follow_up.generation),
            attempt,
        );
        let Some(next) = self.repository.change_follow_up(
            follow_up.conversation_id,
            follow_up.target,
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
        let mut failure = None;
        for follow_up in self.repository.follow_ups_in(&[
            SceneFollowUpState::Approved,
            SceneFollowUpState::Running,
            SceneFollowUpState::AwaitingTurn,
            SceneFollowUpState::Pending,
        ])? {
            let recovered = (|| {
                if follow_up.state == SceneFollowUpState::AwaitingTurn {
                    return complete_one(self.repository, &follow_up, now);
                }
                if follow_up.state == SceneFollowUpState::Pending {
                    if follow_up.mode != SceneFollowUpMode::Auto {
                        return Ok(false);
                    }
                    self.start(
                        follow_up.conversation_id,
                        follow_up.target,
                        &[SceneFollowUpState::Pending],
                        None,
                        None,
                        now,
                    )?;
                    return Ok(true);
                }
                let Some(request_id) = follow_up.request_id else {
                    self.fail(&follow_up, labels::SCENE_IMAGE_INTERRUPTED, now)?;
                    return Ok(true);
                };
                match self.job_of_request(request_id)? {
                    None if follow_up.state == SceneFollowUpState::Approved => {
                        self.admit_job(&follow_up, now)?;
                        Ok(true)
                    }
                    None => {
                        self.fail(&follow_up, labels::SCENE_IMAGE_INTERRUPTED, now)?;
                        Ok(true)
                    }
                    Some(job) if job.state.is_terminal() => {
                        let record = ImageGenerationRepository::get(self.repository, job.id)
                            .map_err(|_| SceneFollowUpError::Storage)?;
                        self.settle(&record, true, now)?;
                        Ok(true)
                    }
                    Some(_) => Ok(false),
                }
            })();
            match recovered {
                Ok(true) => changed += 1,
                Ok(false) => {}
                Err(error) => {
                    let label = match &error {
                        SceneFollowUpError::Image(error) => failure_label(error),
                        _ => labels::SCENE_IMAGE_FAILED,
                    };
                    match self.fail(&follow_up, label, now) {
                        Ok(_) => changed += 1,
                        Err(storage) => {
                            failure.get_or_insert(storage);
                        }
                    }
                    if matches!(
                        error,
                        SceneFollowUpError::Storage
                            | SceneFollowUpError::Image(
                                SceneImageError::Storage
                                    | SceneImageError::Model(ImageFeatureModelError::Storage)
                            )
                    ) {
                        failure.get_or_insert(error);
                    }
                }
            }
        }
        if let Some(error) = failure {
            Err(error)
        } else {
            Ok(changed)
        }
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

pub(crate) fn complete_awaiting<R: SceneFollowUpSources + ?Sized>(
    repository: &R,
    now: TimestampMillis,
) -> Result<usize, SceneFollowUpError> {
    let mut changed = 0;
    let mut failure = None;
    for follow_up in repository.follow_ups_in(&[SceneFollowUpState::AwaitingTurn])? {
        match complete_one(repository, &follow_up, now) {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(error) => {
                if failure.is_none() {
                    failure = Some(error);
                }
            }
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(changed),
    }
}

fn complete_one<R: SceneFollowUpSources + ?Sized>(
    repository: &R,
    follow_up: &SceneFollowUp,
    now: TimestampMillis,
) -> Result<bool, SceneFollowUpError> {
    let failed = |label: &str| -> Result<bool, SceneFollowUpError> {
        repository.change_follow_up(
            follow_up.conversation_id,
            follow_up.target,
            &[SceneFollowUpState::AwaitingTurn],
            &SceneFollowUpChange {
                state: Some(SceneFollowUpState::Failed),
                failure: Some(Some(label.to_owned())),
                ..SceneFollowUpChange::default()
            },
            now,
        )?;
        Ok(true)
    };
    let Some(request_id) = follow_up.request_id else {
        return failed(labels::SCENE_IMAGE_INTERRUPTED);
    };
    let Some(job) = crate::job_of_scene_image_request(repository, request_id)? else {
        return failed(labels::SCENE_IMAGE_INTERRUPTED);
    };
    let record = match ImageGenerationRepository::get(repository, job.id) {
        Ok(record) => record,
        Err(lettuce_image_generation::ImageGenerationRepositoryError::Storage) => {
            return Err(SceneFollowUpError::Storage);
        }
        Err(_) => return failed(labels::SCENE_IMAGE_INTERRUPTED),
    };
    let ImageGenerationState::Succeeded { result } = record.state else {
        return failed(labels::SCENE_IMAGE_INTERRUPTED);
    };
    let Some(image) = result.images.first() else {
        return failed(labels::SCENE_IMAGE_NO_IMAGE);
    };
    match crate::image::scene_image::attach_scene_image_phase(
        repository,
        follow_up.conversation_id,
        follow_up.message_id,
        follow_up.target,
        crate::image::scene_image::SceneAttachmentPhase::Deferred(root_request(
            follow_up.target,
            follow_up.generation,
        )),
        image.asset_id,
        now,
    ) {
        Ok(_) => Ok(true),
        Err(SceneImageError::MessageUnavailable) => {
            if lettuce_conversations::ConversationOverviewReader::live_turn(
                repository,
                follow_up.conversation_id,
            )
            .map_err(|_| SceneFollowUpError::Storage)?
            .is_some()
            {
                Ok(false)
            } else {
                failed(labels::SCENE_IMAGE_MESSAGE_UNAVAILABLE)
            }
        }
        Err(error) => Err(error.into()),
    }
}
