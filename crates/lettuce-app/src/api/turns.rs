//! The operations that begin a generation turn besides a send: regenerate,
//! continue and retry, and the director's user message that begins none.
//! Each runs the same guards before it writes: the chat has no live turn,
//! the optional models the operation uses are installed, the memory
//! rewinds a delete owes are finished, and a group has a row for every
//! member. A repeated request replays its first result.

use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    BeginGeneration, ContinueConversation, Conversation, ConversationLifecycle,
    ConversationOverviewReader, ConversationReader, ConversationRepository,
    ConversationRepositoryError, GenerationTurnStatus, MessageDraft, MessagePart,
    MessageRenderSource, MessageRole, MessageVisibility, OperationKind, OperationToken,
    RegenerateCandidate, RetryGeneration, SendConversation,
};
use lettuce_types::{
    ConversationId, ConversationParticipantId, GenerationTurnId, MessageId, Revision,
};

use super::ApiContext;
use super::conversations::schedule_begun;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::events::GenerationEventSink;
use super::messages::{
    changed, committed_revision, current_message, delete_after_error, expected_revision,
    on_timeline, operation, replayed,
};
use super::models::{TurnOperation, require_conversation_models};
use crate::{CompanionEmotionEngine, CompanionTurnCoordinator, CompanionTurnError};

/// A conversation that can begin a request: it exists and is not deleted.
fn writable(
    context: &ApiContext,
    conversation_id: ConversationId,
) -> Result<Conversation, ApiError> {
    let conversation = ConversationReader::get(context.backend().database(), conversation_id)
        .map_err(IntoApiError::into_api_error)?
        .conversation;
    refuse_deleted(conversation)
}

pub(super) fn refuse_deleted(conversation: Conversation) -> Result<Conversation, ApiError> {
    if conversation.lifecycle == ConversationLifecycle::Tombstoned {
        return Err(api_error(
            ApiErrorCode::NotFound,
            "the conversation was deleted",
        ));
    }
    Ok(conversation)
}

fn busy() -> ApiError {
    api_error(
        ApiErrorCode::Busy,
        "the conversation is still generating a reply",
    )
}

fn refuse_live_turn(context: &ApiContext, conversation_id: ConversationId) -> Result<(), ApiError> {
    match ConversationOverviewReader::live_turn(context.backend().database(), conversation_id)
        .map_err(IntoApiError::into_api_error)?
    {
        Some(_) => Err(busy()),
        None => Ok(()),
    }
}

/// A begin refused because another turn is live is `Busy`, not a generic
/// conflict.
fn begin_error(
    context: &ApiContext,
    conversation_id: ConversationId,
    error: ConversationRepositoryError,
) -> ApiError {
    match error {
        ConversationRepositoryError::Conflict
        | ConversationRepositoryError::StaleRevision { .. }
            if ConversationOverviewReader::live_turn(
                context.backend().database(),
                conversation_id,
            )
            .is_ok_and(|turn| turn.is_some()) =>
        {
            busy()
        }
        error => error.into_api_error(),
    }
}

fn companion_begin_error(
    context: &ApiContext,
    conversation_id: ConversationId,
    error: CompanionTurnError,
) -> ApiError {
    match error {
        CompanionTurnError::Conversation(error) => begin_error(context, conversation_id, error),
        error => error.into_api_error(),
    }
}

/// Finishes the memory rewinds a delete owes this conversation. One that
/// still fails refuses the request with `PendingMemoryRewind`: nothing
/// generates on top of memory that is ahead of the chat.
pub(super) async fn finish_owed_rewinds(
    context: &ApiContext,
    conversation_id: ConversationId,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            let database = context.backend().database();
            crate::DynamicMemoryDeleteAfterCoordinator::new(database, database)
                .complete_pending(Some(conversation_id), context.now())
                .and_then(|report| report.into_result().map(|_| ()))
                .map_err(|error| delete_after_error(context, conversation_id, error))
        })
        .await?;
    context.jobs().wake();
    Ok(())
}

/// What a request checks before it writes: the chat exists, the models the
/// operation uses are installed, no turn is live, and the memory rewinds a
/// delete owes are finished.
pub(super) async fn preflight(
    context: &ApiContext,
    conversation_id: ConversationId,
    operation: TurnOperation,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| writable(context, conversation_id).map(|_| ()))
        .await?;
    require_conversation_models(context, conversation_id, operation).await?;
    context
        .blocking(move |context| refuse_live_turn(context, conversation_id))
        .await?;
    finish_owed_rewinds(context, conversation_id).await
}

/// The revision a request under `client` begins from: the chat must still be
/// at the revision the caller last saw. Adding the group's new members
/// changes the revision but nothing the caller could have seen, so the
/// request continues from the revision after it.
fn begin_revision(
    context: &ApiContext,
    conversation_id: ConversationId,
    client: Revision,
) -> Result<Revision, ApiError> {
    begin_revision_with(context, conversation_id, client, || {})
}

fn begin_revision_with(
    context: &ApiContext,
    conversation_id: ConversationId,
    client: Revision,
    checked: impl FnOnce(),
) -> Result<Revision, ApiError> {
    let database = context.backend().database();
    let seen = writable(context, conversation_id)?.revision;
    if seen != client {
        return Err(api_error(
            ApiErrorCode::Conflict,
            "the conversation changed since it was read",
        ));
    }
    checked();
    let written = crate::conversation::ensure_group_members_at(
        database,
        conversation_id,
        client,
        context.now(),
    )
    .map_err(super::conversation_settings::edit_error)?;
    Ok(written.unwrap_or(client))
}

fn text_field(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn participant(value: Option<String>) -> Result<Option<ConversationParticipantId>, ApiError> {
    value
        .map(|id| parse_id(&id, "forced_speaker_participant_id"))
        .transpose()
}

fn optional_bytes(value: Option<&str>) -> Vec<u8> {
    match value {
        Some(value) => [&[1_u8][..], value.as_bytes()].concat(),
        None => vec![0],
    }
}

/// Generates another variant of a reply. The message must be an assistant
/// reply of the selected branch that was not deleted; in a one-to-one chat
/// it must be the newest message. A group reply is spoken again by the
/// member who spoke it unless `forced_speaker_participant_id` names another
/// member. A reply that was edited regenerates from the variant it edited.
pub async fn conversation_regenerate(
    context: &ApiContext,
    request: dto::ConversationRegenerateRequest,
    events: Arc<dyn GenerationEventSink>,
) -> Result<dto::GenerationAccepted, ApiError> {
    regenerate_with(context, request, events, |context, begun, now| {
        context
            .backend()
            .conversation_generation_dispatcher()
            .schedule(begun, now)
    })
    .await
}

pub(super) async fn regenerate_with<S>(
    context: &ApiContext,
    request: dto::ConversationRegenerateRequest,
    events: Arc<dyn GenerationEventSink>,
    schedule: S,
) -> Result<dto::GenerationAccepted, ApiError>
where
    S: FnOnce(
            &ApiContext,
            &BeginGeneration,
            lettuce_types::TimestampMillis,
        ) -> Result<
            crate::ConversationGenerationAdmission,
            crate::ConversationGenerationDispatchError,
        > + Send
        + 'static,
{
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let client_revision = expected_revision(request.expected_revision)?;
    let guidance = text_field(request.guidance);
    let model_profile_id = text_field(request.model_profile_id)
        .map(|id| parse_id::<lettuce_types::ModelProfileId>(&id, "model_profile_id"))
        .transpose()?;
    let forced_speaker = participant(request.forced_speaker_participant_id)?;
    let swap_roles = request.swap_places;
    let operation = operation(
        request.client_operation_id,
        &[
            b"lettuce-conversation-regenerate-v1",
            conversation_id.to_string().as_bytes(),
            message_id.to_string().as_bytes(),
            &optional_bytes(guidance.as_deref()),
            &optional_bytes(model_profile_id.map(|id| id.to_string()).as_deref()),
            &optional_bytes(forced_speaker.map(|id| id.to_string()).as_deref()),
            &[u8::from(swap_roles)],
        ],
    )?;
    let replay = {
        let operation = operation.clone();
        context
            .blocking(move |context| {
                replayed(
                    context,
                    conversation_id,
                    OperationKind::Regenerate,
                    &operation,
                )
            })
            .await?
    };
    if !replay {
        preflight(context, conversation_id, TurnOperation::Regenerate).await?;
    }
    let accepted = context
        .blocking(move |context| {
            let database = context.backend().database();
            let now = context.now();
            let expected = if replay {
                client_revision
            } else {
                begin_revision(context, conversation_id, client_revision)?
            };
            let conversation = writable(context, conversation_id)?;
            let branch_id = conversation.active_branch_id;
            let command = if replay {
                replay_command(context, conversation_id, &operation, expected)?
            } else {
                let item = on_timeline(context, conversation_id, branch_id, message_id)?.item;
                if item.message.visibility == MessageVisibility::Tombstoned {
                    return Err(api_error(ApiErrorCode::NotFound, "the message was deleted"));
                }
                if item.message.role != MessageRole::Assistant {
                    return Err(invalid_field("message_id", "the message is not a reply"));
                }
                if item.message.branch_id != branch_id {
                    return Err(api_error(
                        ApiErrorCode::Unsupported,
                        "the reply belongs to a parent branch; select that branch to regenerate it",
                    ));
                }
                let candidate_id = match (&item.message.active_render_source, &item.active_revision)
                {
                    (MessageRenderSource::Candidate(id), _) => *id,
                    (MessageRenderSource::Revision(_), Some(revision)) => {
                        revision.supersedes_candidate_id.ok_or_else(|| {
                            invalid_field("message_id", "the reply has nothing to regenerate from")
                        })?
                    }
                    (MessageRenderSource::Revision(_), None) => {
                        return Err(api_error(
                            ApiErrorCode::Internal,
                            "the reply's shown edit is missing",
                        ));
                    }
                };
                let candidate = ConversationReader::get_candidate(database, candidate_id)
                    .map_err(IntoApiError::into_api_error)?;
                let source = ConversationReader::get_turn(database, candidate.turn_id)
                    .map_err(IntoApiError::into_api_error)?;
                let model_override = model_profile_id
                    .map(|id| {
                        crate::conversation::conversation_model(database, conversation_id, id)
                            .map_err(super::conversation_settings::edit_error)
                    })
                    .transpose()?;
                RegenerateCandidate {
                    conversation_id,
                    branch_id,
                    message_id,
                    turn_id: source.id,
                    expected_revision: expected,
                    expected_turn_revision: source.revision,
                    operation: operation.clone(),
                    active_candidate_id: candidate_id,
                    guidance,
                    model_override,
                    forced_speaker,
                    swap_roles,
                }
            };
            let begun = ConversationRepository::begin_regenerate(database, &command, now)
                .map_err(|error| begin_error(context, conversation_id, error))?
                .value;
            let accepted = dto::GenerationAccepted {
                turn_id: begun.turn.id.to_string(),
            };
            schedule_begun(context, &begun, events, schedule, now)?;
            Ok(accepted)
        })
        .await?;
    context.wake_workers();
    Ok(accepted)
}

/// The command a repeated regenerate replays: the repository answers from
/// the recorded operation and reads nothing else of it.
fn replay_command(
    context: &ApiContext,
    conversation_id: ConversationId,
    operation: &OperationToken,
    expected: Revision,
) -> Result<RegenerateCandidate, ApiError> {
    let record = ConversationReader::operation_record(
        context.backend().database(),
        conversation_id,
        OperationKind::Regenerate,
        operation,
    )
    .map_err(IntoApiError::into_api_error)?
    .ok_or_else(|| api_error(ApiErrorCode::Internal, "the recorded operation is missing"))?;
    let lettuce_conversations::OperationResultRef::Turn(turn_id) = record.result else {
        return Err(api_error(
            ApiErrorCode::Internal,
            "the recorded operation is not a turn",
        ));
    };
    let turn = ConversationReader::get_turn(context.backend().database(), turn_id)
        .map_err(IntoApiError::into_api_error)?;
    let lettuce_conversations::GenerationTarget::ExistingCandidate {
        message_id,
        prior_candidate_id,
    } = turn.target
    else {
        return Err(api_error(
            ApiErrorCode::Internal,
            "the recorded turn is not a regeneration",
        ));
    };
    Ok(RegenerateCandidate {
        conversation_id,
        branch_id: turn.branch_id,
        message_id,
        turn_id,
        expected_revision: expected,
        expected_turn_revision: turn.revision,
        operation: operation.clone(),
        active_candidate_id: prior_candidate_id,
        guidance: turn.guidance,
        model_override: turn.requested_model_override,
        forced_speaker: turn.forced_speaker,
        swap_roles: turn.swap_roles,
    })
}

/// Generates a new reply after the newest message of the selected branch.
/// A group reply is spoken by `forced_speaker_participant_id` when given,
/// else by the chat's speaker selection. A companion chat's state is not
/// updated by a continue.
pub async fn conversation_continue(
    context: &ApiContext,
    request: dto::ConversationContinueRequest,
    events: Arc<dyn GenerationEventSink>,
) -> Result<dto::GenerationAccepted, ApiError> {
    continue_with(context, request, events, |context, begun, now| {
        context
            .backend()
            .conversation_generation_dispatcher()
            .schedule(begun, now)
    })
    .await
}

pub(super) async fn continue_with<S>(
    context: &ApiContext,
    request: dto::ConversationContinueRequest,
    events: Arc<dyn GenerationEventSink>,
    schedule: S,
) -> Result<dto::GenerationAccepted, ApiError>
where
    S: FnOnce(
            &ApiContext,
            &BeginGeneration,
            lettuce_types::TimestampMillis,
        ) -> Result<
            crate::ConversationGenerationAdmission,
            crate::ConversationGenerationDispatchError,
        > + Send
        + 'static,
{
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let client_revision = expected_revision(request.expected_revision)?;
    let forced_speaker = participant(request.forced_speaker_participant_id)?;
    let swap_roles = request.swap_places;
    let operation = operation(
        request.client_operation_id,
        &[
            b"lettuce-conversation-continue-v1",
            conversation_id.to_string().as_bytes(),
            &optional_bytes(forced_speaker.map(|id| id.to_string()).as_deref()),
            &[u8::from(swap_roles)],
        ],
    )?;
    let replay = {
        let operation = operation.clone();
        context
            .blocking(move |context| {
                replayed(
                    context,
                    conversation_id,
                    OperationKind::Continue,
                    &operation,
                )
            })
            .await?
    };
    if !replay {
        preflight(context, conversation_id, TurnOperation::Continue).await?;
    }
    let accepted = context
        .blocking(move |context| {
            let database = context.backend().database();
            let now = context.now();
            let expected = if replay {
                client_revision
            } else {
                begin_revision(context, conversation_id, client_revision)?
            };
            let conversation = writable(context, conversation_id)?;
            let branch = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?
                .branches
                .into_iter()
                .find(|branch| branch.id == conversation.active_branch_id)
                .ok_or_else(|| {
                    api_error(
                        ApiErrorCode::Internal,
                        "the conversation's active branch is missing",
                    )
                })?;
            if !replay && branch.head_message_id.or(branch.fork_message_id).is_none() {
                return Err(invalid_field(
                    "conversation_id",
                    "the conversation has no message to continue from",
                ));
            }
            let command = ContinueConversation {
                conversation_id,
                branch_id: branch.id,
                expected_revision: expected,
                forced_speaker,
                swap_roles,
                operation,
            };
            let begun =
                CompanionTurnCoordinator::<_, dyn CompanionEmotionEngine>::new(database, None)
                    .begin_continue(&command, now)
                    .map_err(|error| companion_begin_error(context, conversation_id, error))?
                    .value;
            let accepted = dto::GenerationAccepted {
                turn_id: begun.turn.id.to_string(),
            };
            schedule_begun(context, &begun, events, schedule, now)?;
            Ok(accepted)
        })
        .await?;
    context.wake_workers();
    Ok(accepted)
}

/// Starts a new turn that repeats one that failed or was cancelled, with the
/// same target, guidance, model and speaker. A turn that succeeded or is
/// still running is `Conflict`; a turn of another conversation or branch is
/// `NotFound`.
pub async fn conversation_retry(
    context: &ApiContext,
    request: dto::ConversationRetryRequest,
    events: Arc<dyn GenerationEventSink>,
) -> Result<dto::GenerationAccepted, ApiError> {
    retry_with(context, request, events, |context, begun, now| {
        context
            .backend()
            .conversation_generation_dispatcher()
            .schedule(begun, now)
    })
    .await
}

pub(super) async fn retry_with<S>(
    context: &ApiContext,
    request: dto::ConversationRetryRequest,
    events: Arc<dyn GenerationEventSink>,
    schedule: S,
) -> Result<dto::GenerationAccepted, ApiError>
where
    S: FnOnce(
            &ApiContext,
            &BeginGeneration,
            lettuce_types::TimestampMillis,
        ) -> Result<
            crate::ConversationGenerationAdmission,
            crate::ConversationGenerationDispatchError,
        > + Send
        + 'static,
{
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let turn_id: GenerationTurnId = parse_id(&request.turn_id, "turn_id")?;
    let operation = operation(
        request.client_operation_id,
        &[
            b"lettuce-conversation-retry-v1",
            conversation_id.to_string().as_bytes(),
            turn_id.to_string().as_bytes(),
        ],
    )?;
    let replay = {
        let operation = operation.clone();
        context
            .blocking(move |context| {
                replayed(context, conversation_id, OperationKind::Retry, &operation)
            })
            .await?
    };
    if !replay {
        preflight(context, conversation_id, TurnOperation::Retry).await?;
    }
    let accepted = context
        .blocking(move |context| {
            let database = context.backend().database();
            let now = context.now();
            let conversation = writable(context, conversation_id)?;
            if !replay {
                crate::conversation::ensure_group_members(database, conversation_id, now)
                    .map_err(super::conversation_settings::edit_error)?;
            }
            let conversation = if replay {
                conversation
            } else {
                writable(context, conversation_id)?
            };
            let source = ConversationReader::get_turn(database, turn_id)
                .map_err(IntoApiError::into_api_error)?;
            if source.conversation_id != conversation_id
                || source.branch_id != conversation.active_branch_id
            {
                return Err(api_error(
                    ApiErrorCode::NotFound,
                    "the turn is not part of the conversation's selected branch",
                ));
            }
            if !replay
                && !matches!(
                    source.status,
                    GenerationTurnStatus::Failed | GenerationTurnStatus::Cancelled
                )
            {
                return Err(api_error(
                    ApiErrorCode::Conflict,
                    "only a failed or cancelled turn can be retried",
                ));
            }
            let command = RetryGeneration {
                conversation_id,
                branch_id: source.branch_id,
                turn_id,
                expected_revision: conversation.revision,
                expected_turn_revision: source.revision,
                operation,
            };
            let begun = ConversationRepository::begin_retry(database, &command, now)
                .map_err(|error| begin_error(context, conversation_id, error))?
                .value;
            let accepted = dto::GenerationAccepted {
                turn_id: begun.turn.id.to_string(),
            };
            schedule_begun(context, &begun, events, schedule, now)?;
            Ok(accepted)
        })
        .await?;
    context.wake_workers();
    Ok(accepted)
}

/// Adds a user message to the selected branch without generating a reply.
/// The text is trimmed and must not be blank. A turn still running is
/// `Busy`.
pub async fn conversation_add_user_message(
    context: &ApiContext,
    request: dto::ConversationAddUserMessageRequest,
) -> Result<dto::MessageChanged, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let text = request.text.trim().to_owned();
    if text.is_empty() {
        return Err(invalid_field("text", "the message text is blank"));
    }
    let client_revision = expected_revision(request.expected_revision)?;
    let operation = operation(
        request.client_operation_id,
        &[
            b"lettuce-conversation-add-user-message-v1",
            conversation_id.to_string().as_bytes(),
            text.as_bytes(),
        ],
    )?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            if replayed(
                context,
                conversation_id,
                OperationKind::AppendMessage,
                &operation,
            )? {
                let record = ConversationReader::operation_record(
                    database,
                    conversation_id,
                    OperationKind::AppendMessage,
                    &operation,
                )
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| {
                    api_error(ApiErrorCode::Internal, "the recorded operation is missing")
                })?;
                let lettuce_conversations::OperationResultRef::Message(message_id) = record.result
                else {
                    return Err(api_error(
                        ApiErrorCode::Internal,
                        "the recorded operation is not a message",
                    ));
                };
                return current_message(context, conversation_id, message_id);
            }
            let conversation = writable(context, conversation_id)?;
            refuse_live_turn(context, conversation_id)?;
            let user = conversation
                .participants
                .iter()
                .find(|participant| {
                    participant.role == lettuce_conversations::ParticipantRole::User
                })
                .ok_or_else(|| {
                    api_error(
                        ApiErrorCode::Unsupported,
                        "the conversation has no user participant",
                    )
                })?;
            let appended = ConversationRepository::append_user_message(
                database,
                &SendConversation {
                    conversation_id,
                    branch_id: conversation.active_branch_id,
                    expected_revision: client_revision,
                    operation,
                    message: MessageDraft {
                        role: MessageRole::User,
                        author_participant_id: Some(user.id),
                        parts: vec![MessagePart::Text { text }],
                        visibility: MessageVisibility::Visible,
                        pinned: false,
                        scene_edited: false,
                    },
                    swap_roles: false,
                },
                context.now(),
            )
            .map_err(|error| begin_error(context, conversation_id, error))?;
            let revision = committed_revision(&appended.outbox);
            changed(context, appended.value, revision)
        })
        .await
}

#[cfg(test)]
mod revision_race_tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn a_writer_after_the_client_check_is_not_adopted() {
        let harness = crate::api::tests::harness(crate::api::tests::Reply::Text("Hello."));
        let (chat, _) = crate::api::turns_tests::replied_chat(&harness, "revision-race").await;
        let conversation_id: ConversationId = chat.parse().expect("conversation id");
        harness
            .context
            .blocking(move |context| {
                let database = context.backend().database();
                let before = writable(context, conversation_id).expect("conversation");
                let error = begin_revision_with(context, conversation_id, before.revision, || {
                    database
                        .rename(
                            &lettuce_conversations::RenameConversation {
                                conversation_id,
                                expected_revision: before.revision,
                                operation: crate::conversation::edit_operation(
                                    "concurrent-rename".into(),
                                    &[b"rename"],
                                )
                                .expect("token"),
                                title: "Concurrent writer".into(),
                            },
                            context.now(),
                        )
                        .expect("concurrent writer");
                })
                .expect_err("the stale client revision must conflict");
                assert_eq!(error.code, ApiErrorCode::Conflict);
                Ok(())
            })
            .await
            .expect("race assertion");
    }
}
