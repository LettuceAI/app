//! Scene images of a one-to-one chat's replies. A reply that asks for one
//! keeps a follow-up (see `lettuce_conversations::SceneFollowUp`): an
//! automatic one starts when the reply is final, an ask-first one waits for
//! `message_scene_image_approve` or `message_scene_image_dismiss`, and the
//! user can also ask for one from a message. Each start queues an image job
//! the job runner claims; the image lands as an attachment of the message and
//! `ApiEvent::MessageSceneImageChanged` follows every state change.

use std::collections::HashMap;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    ConversationKind, ConversationReader, MessageRole, MessageVisibility, SceneFollowUp,
    SceneFollowUpChange, SceneFollowUpMode, SceneFollowUpRepository, SceneFollowUpState,
};
use lettuce_types::{ConversationId, JobId, MessageId};

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::messages::on_timeline;
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
        SceneFollowUpState::Done => dto::SceneImageState::Done,
        SceneFollowUpState::Failed => dto::SceneImageState::Failed,
        SceneFollowUpState::Dismissed => dto::SceneImageState::Dismissed,
    }
}

fn failure_of(label: &str) -> dto::SceneImageFailure {
    match label {
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
fn view(context: &ApiContext, follow_up: &SceneFollowUp) -> Option<dto::SceneImageView> {
    if matches!(
        follow_up.state,
        SceneFollowUpState::Done | SceneFollowUpState::Dismissed
    ) {
        return None;
    }
    let job_id = follow_up.request_id.and_then(|request_id| {
        crate::job_of_scene_image_request(context.backend().database(), request_id)
            .ok()
            .flatten()
            .map(|job| job.id.to_string())
    });
    Some(dto::SceneImageView {
        state: follow_up_state(follow_up.state),
        mode: match follow_up.mode {
            SceneFollowUpMode::Auto => dto::SceneImageMode::Auto,
            SceneFollowUpMode::AskFirst => dto::SceneImageMode::AskFirst,
            SceneFollowUpMode::Manual => dto::SceneImageMode::Manual,
        },
        prompt: follow_up.prompt.clone(),
        job_id,
        failure: follow_up.failure.as_deref().map(failure_of),
    })
}

/// The scene images of `message_ids` that are still to show.
pub(super) fn views(
    context: &ApiContext,
    conversation_id: ConversationId,
    message_ids: &[MessageId],
) -> Result<HashMap<MessageId, dto::SceneImageView>, ApiError> {
    Ok(SceneFollowUpRepository::follow_ups_of(
        context.backend().database(),
        conversation_id,
        message_ids,
    )
    .map_err(|_| storage())?
    .into_iter()
    .filter_map(|follow_up| view(context, &follow_up).map(|view| (follow_up.message_id, view)))
    .collect())
}

fn image_error(error: SceneImageFollowUpError) -> ApiError {
    match error {
        SceneImageFollowUpError::NotFound => api_error(
            ApiErrorCode::NotFound,
            "the message has no scene image to start",
        ),
        SceneImageFollowUpError::WrongState(state) => api_error(
            match state {
                SceneFollowUpState::Approved | SceneFollowUpState::Running => ApiErrorCode::Busy,
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
        SceneImageFollowUpError::Image(SceneImageError::NotDirect) => api_error(
            ApiErrorCode::Unsupported,
            "scene images are generated for one-to-one chats only",
        ),
        SceneImageFollowUpError::Image(SceneImageError::MessageNotFound) => {
            api_error(ApiErrorCode::NotFound, "the message was not found")
        }
        SceneImageFollowUpError::Image(error) => {
            api_error(ApiErrorCode::Internal, error.to_string())
        }
        SceneImageFollowUpError::Storage => storage(),
    }
}

/// A reply of a one-to-one chat's selected branch that a scene image can
/// be made for.
fn scene_target(context: &ApiContext, message_id: MessageId) -> Result<ConversationId, ApiError> {
    let database = context.backend().database();
    let conversation_id = conversation_of(context, message_id)?;
    let conversation = ConversationReader::get(database, conversation_id)
        .map_err(IntoApiError::into_api_error)?
        .conversation;
    if !matches!(conversation.kind, ConversationKind::Direct(_)) {
        return Err(api_error(
            ApiErrorCode::Unsupported,
            "scene images are generated for one-to-one chats only",
        ));
    }
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
    Ok(conversation_id)
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
    let job_id = context
        .blocking(move |context| {
            let conversation_id = scene_target(context, message_id)?;
            let media = context.media().ok_or_else(media_missing)?;
            let database = context.backend().database();
            database
                .ensure_follow_up(
                    conversation_id,
                    message_id,
                    &prompt,
                    SceneFollowUpMode::Manual,
                    context.now(),
                )
                .map_err(|_| storage())?;
            let (_, job) = SceneFollowUps::new(database, media)
                .start(
                    conversation_id,
                    message_id,
                    &[
                        SceneFollowUpState::Pending,
                        SceneFollowUpState::Done,
                        SceneFollowUpState::Failed,
                        SceneFollowUpState::Dismissed,
                    ],
                    Some(&prompt),
                    Some(SceneFollowUpMode::Manual),
                    context.now(),
                )
                .map_err(image_error)?;
            Ok(job.id)
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
    let job_id = context
        .blocking(move |context| {
            let conversation_id = scene_target(context, message_id)?;
            let media = context.media().ok_or_else(media_missing)?;
            let database = context.backend().database();
            let follow_ups = SceneFollowUps::new(database, media);
            match follow_ups.start(
                conversation_id,
                message_id,
                &[SceneFollowUpState::Pending],
                prompt.as_deref(),
                None,
                context.now(),
            ) {
                Ok((_, job)) => Ok(job.id),
                Err(SceneImageFollowUpError::WrongState(
                    SceneFollowUpState::Approved | SceneFollowUpState::Running,
                )) => {
                    let follow_up = database
                        .get_follow_up(conversation_id, message_id)
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
            let conversation_id = conversation_of(context, message_id)?;
            let database = context.backend().database();
            let Some(follow_up) = database
                .get_follow_up(conversation_id, message_id)
                .map_err(|_| storage())?
            else {
                return Ok(());
            };
            match follow_up.state {
                SceneFollowUpState::Dismissed | SceneFollowUpState::Done => Ok(()),
                SceneFollowUpState::Approved | SceneFollowUpState::Running => Err(api_error(
                    ApiErrorCode::Conflict,
                    "the scene image is being generated; cancel its job instead",
                )),
                SceneFollowUpState::Pending | SceneFollowUpState::Failed => {
                    database
                        .change_follow_up(
                            conversation_id,
                            message_id,
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
    request: dto::MessageSceneRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let conversation_id = context
        .blocking(move |context| scene_target(context, message_id))
        .await?;
    super::jobs::admit_scene_prompt(
        context,
        conversation_id,
        message_id,
        uuid::Uuid::new_v4().to_string(),
    )
    .await
}

/// Starts the scene image of a finalized reply whose follow-up is automatic.
/// Called after a turn completed; a follow-up that is not automatic and
/// pending, or a chat without a media store, is left alone.
pub(super) fn start_auto(
    context: &ApiContext,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<(), ApiError> {
    let database = context.backend().database();
    let Some(follow_up) = database
        .get_follow_up(conversation_id, message_id)
        .map_err(|_| storage())?
    else {
        return Ok(());
    };
    if follow_up.state != SceneFollowUpState::Pending || follow_up.mode != SceneFollowUpMode::Auto {
        return Ok(());
    }
    let Some(media) = context.media() else {
        return Ok(());
    };
    match SceneFollowUps::new(database, media).start(
        conversation_id,
        message_id,
        &[SceneFollowUpState::Pending],
        None,
        None,
        context.now(),
    ) {
        Ok(_) | Err(SceneImageFollowUpError::WrongState(_)) => {
            context.jobs().wake();
            Ok(())
        }
        Err(error) => {
            tracing::warn!(%error, %message_id, "an automatic scene image could not start");
            Ok(())
        }
    }
}

/// Settles the scene image follow-ups the previous process left unfinished.
pub(super) fn recover(context: &ApiContext) {
    let Some(media) = context.media() else {
        return;
    };
    match SceneFollowUps::new(context.backend().database(), media).recover(context.now()) {
        Ok(0) => {}
        Ok(changed) => {
            tracing::info!(changed, "settled scene images the previous process left");
            context.jobs().wake();
        }
        Err(error) => tracing::warn!(%error, "scene images could not be recovered"),
    }
}
