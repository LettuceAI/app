//! Scene images of a chat's replies. A reply that asks for one
//! keeps a follow-up (see `lettuce_conversations::SceneFollowUp`): an
//! automatic one starts when the reply is final, an ask-first one waits for
//! `message_scene_image_approve` or `message_scene_image_dismiss`, and the
//! user can also ask for one from a message. Each start queues an image job
//! the job runner claims; the image lands as an attachment of the message and
//! `ApiEvent::MessageSceneImageChanged` follows every state change.

use std::collections::HashMap;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    ConversationReader, MessageRole, MessageVisibility, SceneFollowUp,
    SceneFollowUpChange, SceneFollowUpMode, SceneFollowUpRepository, SceneFollowUpState,
    SceneFollowUpTarget,
};
use lettuce_types::{CharacterId, ConversationId, JobId, MessageId};

use super::messages::on_timeline;
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, invalid_field, parse_id};
use crate::jobs::failure_labels as labels;
use crate::{SceneFollowUps, SceneImageError, SceneImageFollowUpError};

fn conversation_of(
    context: &ApiContext,
    message_id: MessageId,
) -> Result<ConversationId, ApiError> {
    lettuce_conversations::ConversationOverviewReader::conversation_of_message(
        context.backend().database(),
        message_id,
    )
    .map_err(IntoApiError::into_api_error)?
    .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the message was not found"))
}

fn storage() -> ApiError {
    api_error(
        ApiErrorCode::Internal,
        "the scene image follow-up could not be read",
    )
}

const fn follow_up_state(state: SceneFollowUpState) -> dto::SceneImageState {
    match state {
        SceneFollowUpState::Pending => dto::SceneImageState::Pending,
        SceneFollowUpState::Approved => dto::SceneImageState::Approved,
        SceneFollowUpState::Running => dto::SceneImageState::Running,
        SceneFollowUpState::AwaitingTurn => dto::SceneImageState::AwaitingTurn,
        SceneFollowUpState::Done => dto::SceneImageState::Done,
        SceneFollowUpState::Failed => dto::SceneImageState::Failed,
        SceneFollowUpState::Dismissed => dto::SceneImageState::Dismissed,
    }
}

fn failure_of(label: &str) -> dto::SceneImageFailure {
    match label {
        labels::SCENE_IMAGE_MEDIA_UNAVAILABLE => dto::SceneImageFailure::MediaUnavailable,
        labels::SCENE_IMAGE_DISABLED => dto::SceneImageFailure::Disabled,
        labels::SCENE_IMAGE_NO_MODEL => dto::SceneImageFailure::NoModel,
        labels::SCENE_IMAGE_NO_IMAGE => dto::SceneImageFailure::NoImage,
        labels::SCENE_IMAGE_MESSAGE_UNAVAILABLE => dto::SceneImageFailure::MessageUnavailable,
        labels::SCENE_IMAGE_INTERRUPTED => dto::SceneImageFailure::Interrupted,
        _ => dto::SceneImageFailure::Failed,
    }
}

/// The follow-up as the message shows it: not once its image is on the
/// message or the user dismissed it.
fn view(
    context: &ApiContext,
    follow_up: &SceneFollowUp,
) -> Result<Option<dto::SceneImageView>, ApiError> {
    if matches!(
        follow_up.state,
        SceneFollowUpState::Done | SceneFollowUpState::Dismissed
    ) {
        return Ok(None);
    }
    let job_id = follow_up
        .request_id
        .map(|request_id| {
            crate::job_of_scene_image_request(context.backend().database(), request_id)
                .map_err(|_| storage())
                .map(|job| job.map(|job| job.id.to_string()))
        })
        .transpose()?
        .flatten();
    Ok(Some(dto::SceneImageView {
        state: follow_up_state(follow_up.state),
        mode: match follow_up.mode {
            SceneFollowUpMode::Auto => dto::SceneImageMode::Auto,
            SceneFollowUpMode::AskFirst => dto::SceneImageMode::AskFirst,
            SceneFollowUpMode::Manual => dto::SceneImageMode::Manual,
        },
        prompt: follow_up.prompt.clone(),
        job_id,
        failure: follow_up.failure.as_deref().map(failure_of),
    }))
}

fn target_of(item: &lettuce_conversations::TimelineItem) -> Option<SceneFollowUpTarget> {
    item.active_candidate
        .as_ref()
        .map(|candidate| SceneFollowUpTarget::Candidate(candidate.id))
        .or_else(|| {
            item.active_revision.as_ref().map(|revision| {
                revision
                    .supersedes_candidate_id
                    .map(SceneFollowUpTarget::Candidate)
                    .unwrap_or(SceneFollowUpTarget::StarterRevision(revision.id))
            })
        })
}

/// The scene images of `message_ids` that are still to show.
pub(super) fn views(
    context: &ApiContext,
    conversation_id: ConversationId,
    items: &[&lettuce_conversations::TimelineItem],
) -> Result<HashMap<MessageId, dto::SceneImageView>, ApiError> {
    let follow_ups = SceneFollowUpRepository::follow_ups_of(
        context.backend().database(),
        conversation_id,
        &items
            .iter()
            .filter_map(|item| target_of(item))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| storage())?;
    let mut views = HashMap::new();
    for follow_up in follow_ups {
        if let Some(view) = view(context, &follow_up)? {
            views.insert(follow_up.message_id, view);
        }
    }
    Ok(views)
}

fn image_error(error: SceneImageFollowUpError) -> ApiError {
    match error {
        SceneImageFollowUpError::NotFound => api_error(
            ApiErrorCode::NotFound,
            "the message has no scene image to start",
        ),
        SceneImageFollowUpError::WrongState(state) => api_error(
            match state {
                SceneFollowUpState::Approved
                | SceneFollowUpState::Running
                | SceneFollowUpState::AwaitingTurn => ApiErrorCode::Busy,
                _ => ApiErrorCode::Conflict,
            },
            format!("the scene image is already {state:?}"),
        ),
        SceneImageFollowUpError::Image(SceneImageError::Model(
            crate::ImageFeatureModelError::SceneDisabled,
        )) => api_error(
            ApiErrorCode::Unsupported,
            "Scene generation is disabled in settings",
        ),
        SceneImageFollowUpError::Image(SceneImageError::Model(_)) => api_error(
            ApiErrorCode::Unavailable,
            "no image model is configured for scenes",
        ),
        SceneImageFollowUpError::Image(SceneImageError::ForeignCastMember) => {
            invalid_field("cast", "the cast names a character that is not in the chat")
        }
        SceneImageFollowUpError::Image(SceneImageError::EmptyCast) => {
            invalid_field("cast", "the scene has nobody to depict")
        }
        SceneImageFollowUpError::Image(SceneImageError::MessageNotFound) => {
            api_error(ApiErrorCode::NotFound, "the message was not found")
        }
        SceneImageFollowUpError::Image(error) => {
            api_error(ApiErrorCode::Internal, error.to_string())
        }
        SceneImageFollowUpError::Storage { .. } => storage(),
    }
}

/// The cast a request names, its ids parsed.
pub(in crate::api) fn cast_of(
    cast: Option<dto::SceneCastSelection>,
) -> Result<Option<lettuce_conversations::SceneCastSelection>, ApiError> {
    cast.map(|cast| {
        Ok(lettuce_conversations::SceneCastSelection {
            character_ids: cast
                .character_ids
                .iter()
                .map(|id| parse_id::<CharacterId>(id, "cast"))
                .collect::<Result<_, _>>()?,
            include_persona: cast.include_persona,
        })
    })
    .transpose()
}

/// A reply of a chat's selected branch that a scene image can be made for.
pub(in crate::api) fn scene_target(
    context: &ApiContext,
    message_id: MessageId,
) -> Result<(ConversationId, SceneFollowUpTarget), ApiError> {
    let database = context.backend().database();
    let conversation_id = conversation_of(context, message_id)?;
    let conversation = ConversationReader::get(database, conversation_id)
        .map_err(IntoApiError::into_api_error)?
        .conversation;
    let item = on_timeline(
        context,
        conversation_id,
        conversation.active_branch_id,
        message_id,
    )?
    .item;
    if item.message.role != MessageRole::Assistant
        || item.message.visibility != MessageVisibility::Visible
    {
        return Err(invalid_field("message_id", "the message is not a reply"));
    }
    let target = target_of(&item)
        .ok_or_else(|| invalid_field("message_id", "the reply has no scene target"))?;
    Ok((conversation_id, target))
}

fn media_missing() -> ApiError {
    api_error(ApiErrorCode::Unavailable, "the media store is unavailable")
}

/// Starts the scene image of a message from `prompt`, trimmed and not blank.
/// The message keeps at most one image job at a time: while one is queued or
/// running the call is `Busy`.
pub async fn message_scene_image_generate(
    context: &ApiContext,
    request: dto::MessageSceneImageGenerateRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let prompt = request.prompt.trim().to_owned();
    if prompt.is_empty() {
        return Err(invalid_field("prompt", "the scene prompt is blank"));
    }
    let cast = cast_of(request.cast)?;
    let key = crate::api::jobs::local::operation_key("scene_image", &request.client_operation_id)?;
    let digest = crate::api::jobs::local::digest(&(message_id.to_string(), &prompt, &cast))?;
    let job_id = context
        .blocking(move |context| {
            let database = context.backend().database();
            if let Some(prior) = database.job_operation(&key).map_err(|_| storage())? {
                if prior.request_digest != digest {
                    return Err(api_error(
                        ApiErrorCode::Conflict,
                        "the operation key names another request",
                    ));
                }
                return Ok(prior.job_id);
            }
            let (conversation_id, target) = scene_target(context, message_id)?;
            let media = context.media().ok_or_else(media_missing)?;
            let request_id =
                lettuce_types::RequestId::from_uuid(crate::api::jobs::local::stable_uuid(&[
                    "scene-image",
                    &key,
                ]));
            let request = crate::scene_generation_request(
                database,
                media,
                &crate::SceneImageRequest {
                    conversation_id,
                    message_id,
                    scene_prompt: prompt.clone(),
                    request_id,
                    cast: cast.clone(),
                },
                context.now(),
            )
            .map_err(|error| image_error(SceneImageFollowUpError::Image(error)))?;
            let (request, spec) = crate::ImageGenerationCoordinator::new(database, database)
                .plan_admission(request, database)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            database
                .admit_manual_scene_image(lettuce_database::ManualSceneImageAdmission {
                    spec,
                    request,
                    operation_key: &key,
                    request_digest: &digest,
                    conversation_id,
                    message_id,
                    target,
                    prompt: &prompt,
                    cast: cast.as_ref(),
                })
                .map(|job| job.id)
                .map_err(|error| match error {
                    lettuce_jobs::StoreError::IdempotencyConflict => api_error(
                        ApiErrorCode::Conflict,
                        "the operation key names another request",
                    ),
                    lettuce_jobs::StoreError::ResourceUnavailable => {
                        api_error(ApiErrorCode::Busy, "the scene image is being generated")
                    }
                    _ => storage(),
                })
        })
        .await?;
    context.jobs().wake();
    Ok(accepted(job_id))
}

fn accepted(job_id: JobId) -> dto::JobAccepted {
    dto::JobAccepted {
        job_id: job_id.to_string(),
    }
}

/// Approves the scene image a reply asked for, with `prompt` when the user
/// edited it (trimmed, never blank). Approving what is already generating
/// returns its job.
pub async fn message_scene_image_approve(
    context: &ApiContext,
    request: dto::MessageSceneImageApproveRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let prompt = request.prompt.map(|prompt| prompt.trim().to_owned());
    if prompt.as_deref().is_some_and(str::is_empty) {
        return Err(invalid_field("prompt", "the scene prompt is blank"));
    }
    let cast = cast_of(request.cast)?;
    let job_id = context
        .blocking(move |context| {
            let (conversation_id, target) = scene_target(context, message_id)?;
            let media = context.media().ok_or_else(media_missing)?;
            let database = context.backend().database();
            let follow_ups = SceneFollowUps::new(database, media);
            match follow_ups.start(
                conversation_id,
                target,
                &[SceneFollowUpState::Pending],
                prompt.as_deref(),
                cast.as_ref(),
                None,
                context.now(),
            ) {
                Ok((_, job)) => Ok(job.id),
                Err(SceneImageFollowUpError::WrongState(
                    SceneFollowUpState::Approved | SceneFollowUpState::Running,
                )) => {
                    let follow_up = database
                        .get_follow_up(conversation_id, target)
                        .map_err(|_| storage())?
                        .and_then(|follow_up| follow_up.request_id);
                    let job = follow_up
                        .map(|request_id| follow_ups.job_of_request(request_id))
                        .transpose()
                        .map_err(image_error)?
                        .flatten();
                    job.map(|job| job.id).ok_or_else(|| {
                        api_error(ApiErrorCode::Busy, "the scene image is being generated")
                    })
                }
                Err(error) => Err(image_error(error)),
            }
        })
        .await?;
    context.jobs().wake();
    Ok(accepted(job_id))
}

/// Dismisses the scene image a reply asked for, or the failure of one; a
/// dismissed or finished one is left alone and an image being generated is
/// `Conflict`.
pub async fn message_scene_image_dismiss(
    context: &ApiContext,
    request: dto::MessageSceneRequest,
) -> Result<(), ApiError> {
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    context
        .blocking(move |context| {
            let (conversation_id, target) = scene_target(context, message_id)?;
            let database = context.backend().database();
            let Some(follow_up) = database
                .get_follow_up(conversation_id, target)
                .map_err(|_| storage())?
            else {
                return Ok(());
            };
            match follow_up.state {
                SceneFollowUpState::Dismissed | SceneFollowUpState::Done => Ok(()),
                SceneFollowUpState::Approved
                | SceneFollowUpState::Running
                | SceneFollowUpState::AwaitingTurn => Err(api_error(
                    ApiErrorCode::Conflict,
                    "the scene image is being generated; cancel its job instead",
                )),
                SceneFollowUpState::Pending | SceneFollowUpState::Failed => {
                    database
                        .change_follow_up(
                            conversation_id,
                            target,
                            &[SceneFollowUpState::Pending, SceneFollowUpState::Failed],
                            &SceneFollowUpChange {
                                state: Some(SceneFollowUpState::Dismissed),
                                ..SceneFollowUpChange::default()
                            },
                            context.now(),
                        )
                        .map_err(|_| storage())?;
                    Ok(())
                }
            }
        })
        .await
}

/// Asks the scene prompt writer for the prompt of a reply's scene image; the
/// prompt is the job's `GeneratedText` result. A disabled scene feature is
/// `Unsupported`.
pub async fn message_scene_prompt_generate(
    context: &ApiContext,
    request: dto::MessageScenePromptGenerateRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let cast = cast_of(request.cast)?;
    let (conversation_id, _) = context
        .blocking(move |context| scene_target(context, message_id))
        .await?;
    crate::api::jobs::admit_scene_prompt(
        context,
        conversation_id,
        message_id,
        cast,
        request.client_operation_id,
    )
    .await
}

/// Everyone a scene image of a reply may depict, with its cast selected:
/// the one its pending scene image names, else the reply's own.
pub async fn message_scene_cast(
    context: &ApiContext,
    request: dto::MessageSceneRequest,
) -> Result<dto::SceneCastView, ApiError> {
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    context
        .blocking(move |context| {
            let (conversation_id, target) = scene_target(context, message_id)?;
            let database = context.backend().database();
            let conversation = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?
                .conversation;
            let stored = database
                .get_follow_up(conversation_id, target)
                .map_err(|_| storage())?
                .and_then(|follow_up| follow_up.cast);
            let chosen = crate::image::scene_cast::scene_cast(
                database,
                &conversation,
                message_id,
                stored.as_ref(),
            )
            .or_else(|error| match error {
                crate::image::scene_cast::SceneCastError::ForeignCharacter
                | crate::image::scene_cast::SceneCastError::Empty
                    if stored.is_some() =>
                {
                    crate::image::scene_cast::scene_cast(database, &conversation, message_id, None)
                }
                error => Err(error),
            })
            .map_err(|error| {
                image_error(SceneImageFollowUpError::Image(SceneImageError::from(error)))
            })?;
            let everyone = crate::image::scene_cast::chat_cast(database, &conversation)
                .map_err(|error| {
                    image_error(SceneImageFollowUpError::Image(SceneImageError::from(error)))
                })?;
            let avatar = |subject: &crate::image::scene_cast::CastSubject| {
                subject
                    .avatars
                    .first()
                    .map(|asset_id| context.asset_ref(*asset_id))
            };
            let mut view = dto::SceneCastView {
                characters: Vec::new(),
                persona: None,
            };
            for subject in &everyone {
                let selected = chosen.subjects.iter().any(|chosen| chosen.kind == subject.kind);
                match subject.kind {
                    crate::image::scene_cast::CastSubjectKind::Character(id) => {
                        view.characters.push(dto::SceneCastCharacter {
                            character_id: id.to_string(),
                            name: subject.name.clone(),
                            avatar: avatar(subject),
                            selected,
                        });
                    }
                    crate::image::scene_cast::CastSubjectKind::Persona(_) => {
                        view.persona = Some(dto::SceneCastPersona {
                            name: subject.name.clone(),
                            avatar: avatar(subject),
                            selected,
                        });
                    }
                }
            }
            Ok(view)
        })
        .await
}

/// Starts the scene image of a finalized reply whose follow-up is automatic.
/// Called after a turn completed; a follow-up that is not automatic and
/// pending, or a chat without a media store, is left alone.
pub(in crate::api) fn start_auto(
    context: &ApiContext,
    conversation_id: ConversationId,
    target: SceneFollowUpTarget,
) -> Result<(), ApiError> {
    let database = context.backend().database();
    let Some(follow_up) = database
        .get_follow_up(conversation_id, target)
        .map_err(|_| storage())?
    else {
        return Ok(());
    };
    if follow_up.state != SceneFollowUpState::Pending || follow_up.mode != SceneFollowUpMode::Auto {
        return Ok(());
    }
    let Some(media) = context.media() else {
        fail_follow_up(
            database,
            &follow_up,
            labels::SCENE_IMAGE_MEDIA_UNAVAILABLE,
            context.now(),
        )?;
        return Ok(());
    };
    match SceneFollowUps::new(database, media).start(
        conversation_id,
        target,
        &[SceneFollowUpState::Pending],
        None,
        None,
        None,
        context.now(),
    ) {
        Ok(_) | Err(SceneImageFollowUpError::WrongState(_)) => {
            context.jobs().wake();
            Ok(())
        }
        Err(error) => {
            let label = match &error {
                SceneImageFollowUpError::Image(error) => {
                    crate::image::scene_follow_up::failure_label(error)
                }
                _ => labels::SCENE_IMAGE_FAILED,
            };
            fail_follow_up(database, &follow_up, label, context.now())?;
            Ok(())
        }
    }
}

pub(in crate::api) fn fail_follow_up(
    repository: &impl SceneFollowUpRepository,
    follow_up: &SceneFollowUp,
    label: &str,
    now: lettuce_types::TimestampMillis,
) -> Result<(), ApiError> {
    repository
        .change_follow_up(
            follow_up.conversation_id,
            follow_up.target,
            &[
                SceneFollowUpState::Pending,
                SceneFollowUpState::Approved,
                SceneFollowUpState::Running,
            ],
            &SceneFollowUpChange {
                state: Some(SceneFollowUpState::Failed),
                failure: Some(Some(label.to_owned())),
                ..SceneFollowUpChange::default()
            },
            now,
        )
        .map_err(|_| storage())?;
    Ok(())
}

/// Settles the scene image follow-ups the previous process left unfinished.
pub(in crate::api) fn recover(context: &ApiContext) -> Result<(), ApiError> {
    let completion = crate::image::scene_follow_up::complete_awaiting(
        context.backend().database(),
        context.now(),
    )
    .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()));
    let Some(media) = context.media() else {
        let database = context.backend().database();
        let follow_ups = database
            .follow_ups_in(&[
                SceneFollowUpState::Approved,
                SceneFollowUpState::Running,
                SceneFollowUpState::Pending,
            ])
            .map_err(|_| storage())?;
        let mut failure = None;
        for follow_up in follow_ups {
            if follow_up.state == SceneFollowUpState::Pending
                && follow_up.mode != SceneFollowUpMode::Auto
            {
                continue;
            }
            if let Err(error) = fail_follow_up(
                database,
                &follow_up,
                labels::SCENE_IMAGE_MEDIA_UNAVAILABLE,
                context.now(),
            ) {
                failure.get_or_insert(error);
            }
        }
        return completion
            .map(|_| ())
            .and_then(|_| failure.map_or(Ok(()), Err));
    };
    let recovery = SceneFollowUps::new(context.backend().database(), media)
        .recover(context.now())
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()));
    if recovery.as_ref().is_ok_and(|changed| *changed > 0) {
        context.jobs().wake();
    }
    completion.and(recovery.map(|_| ()))
}
