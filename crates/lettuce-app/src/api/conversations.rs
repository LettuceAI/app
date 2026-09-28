use std::{collections::HashMap, sync::Arc};

use lettuce_characters::CharacterRepository;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    BeginGeneration, ConversationKindTag, ConversationLifecycle, ConversationOverview,
    ConversationOverviewReader, ConversationQuery, ConversationReader, ConversationRepositoryError,
    GenerationInput, IdempotencyKey, MessageDraft, MessagePart, MessageRole, MessageVisibility,
    OperationKind, OperationToken, ParticipantRole, ParticipantSource, SendConversation,
    ValidationError,
};
use lettuce_jobs::{CancellationReason, handle::CancellationToken};
use lettuce_types::{
    CharacterId, ContentHash, ConversationBranchId, ConversationId, ConversationStarterId,
    GenerationTurnId, GroupId, PageRequest, SceneId, TimestampMillis,
};

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::events::GenerationEventSink;
use super::mapping::{self, AvatarLookup};
use super::models::{MissingModels, TurnOperation};
use super::worker::settled_event;
use crate::{
    CompanionTurnCoordinator, CompanionTurnError, ConversationGenerationAdmission,
    ConversationGenerationCancellationOutcome, ConversationGenerationDispatchError,
    ConversationLaunchError, DIRECT_LAUNCH_REQUEST_FORMAT_V1, DirectConversationLaunchRequest,
    DirectUserParticipant, GROUP_LAUNCH_REQUEST_FORMAT_V1, GroupConversationLaunchRequest,
    LaunchSelection,
};

const DEFAULT_USER_DISPLAY_NAME: &str = "User";

fn page_request(cursor: Option<String>, limit: Option<u32>) -> PageRequest {
    PageRequest {
        cursor,
        limit: mapping::page_limit(limit),
    }
}

fn operation_key(value: String) -> Result<IdempotencyKey, ApiError> {
    IdempotencyKey::new(value).map_err(|_| {
        invalid_field(
            "client_operation_id",
            "client_operation_id is not a valid idempotency key",
        )
    })
}

fn lifecycle_filter(filter: Option<dto::LifecycleFilter>) -> Option<ConversationLifecycle> {
    match filter.unwrap_or_default() {
        dto::LifecycleFilter::Active => Some(ConversationLifecycle::Active),
        dto::LifecycleFilter::Archived => Some(ConversationLifecycle::Archived),
        dto::LifecycleFilter::All => None,
    }
}

const fn kind_filter(kind: dto::ConversationKind) -> ConversationKindTag {
    match kind {
        dto::ConversationKind::Direct => ConversationKindTag::Direct,
        dto::ConversationKind::Group => ConversationKindTag::Group,
    }
}

/// The list row of one conversation. The group chat mode is read live like a
/// turn reads it.
fn conversation_summary(
    context: &ApiContext,
    overview: &ConversationOverview,
    avatars: &mut AvatarLookup,
    missing: &mut MissingModels,
) -> Result<dto::ConversationSummary, ApiError> {
    let database = context.backend().database();
    let conversation = &overview.conversation;
    let mut summary_avatars = Vec::new();
    for participant in &conversation.participants {
        if let ParticipantSource::Character(character_id) = participant.source
            && let Some(asset_id) = avatars
                .avatar(database, character_id)
                .map_err(IntoApiError::into_api_error)?
        {
            summary_avatars.push(context.asset_ref(asset_id));
        }
    }
    Ok(dto::ConversationSummary {
        id: overview.summary.id.to_string(),
        kind: mapping::conversation_kind_tag(overview.summary.kind),
        title: overview.summary.title.clone(),
        avatars: summary_avatars,
        last_message_preview: overview
            .last_message
            .as_ref()
            .and_then(mapping::preview_text),
        updated_at: overview.summary.updated_at.get(),
        archived: overview.summary.lifecycle == ConversationLifecycle::Archived,
        source: mapping::conversation_source(&conversation.kind),
        message_count: overview.message_count,
        chat_mode: chat_mode(context, conversation)?,
        missing_models: missing.of(context, conversation)?,
        memory_blocked: super::messages::memory_blocked(context, conversation.id)?,
    })
}

fn chat_mode(
    context: &ApiContext,
    conversation: &lettuce_conversations::Conversation,
) -> Result<Option<dto::GroupChatMode>, ApiError> {
    Ok(
        crate::generation::live_sources::live_group(context.backend().database(), conversation)
            .map_err(IntoApiError::into_api_error)?
            .map(|group| mapping::group_chat_mode(group.chat_mode)),
    )
}

fn summaries(
    context: &ApiContext,
    overviews: &[ConversationOverview],
) -> Result<Vec<dto::ConversationSummary>, ApiError> {
    let mut avatars = AvatarLookup::default();
    let mut missing = MissingModels::new(context)?;
    overviews
        .iter()
        .map(|overview| conversation_summary(context, overview, &mut avatars, &mut missing))
        .collect()
}

/// Conversations, most recently updated first: active ones unless the
/// request asks for archived ones or all, optionally only one kind, one
/// character's one-to-one chats or one group's chats.
pub async fn conversations_list(
    context: &ApiContext,
    request: dto::ConversationsListRequest,
) -> Result<dto::ConversationPage, ApiError> {
    let character_id: Option<CharacterId> = request
        .character_id
        .as_deref()
        .map(|id| parse_id(id, "character_id"))
        .transpose()?;
    let source_group_id: Option<GroupId> = request
        .source_group_id
        .as_deref()
        .map(|id| parse_id(id, "source_group_id"))
        .transpose()?;
    context
        .blocking(move |context| {
            let page = ConversationOverviewReader::overview_page(
                context.backend().database(),
                &ConversationQuery {
                    lifecycle: lifecycle_filter(request.lifecycle),
                    kind: request.kind.map(kind_filter),
                    character_id,
                    source_group_id,
                    page: page_request(request.cursor, request.limit),
                },
            )
            .map_err(|error| cursor_error(error, "cursor"))?;
            Ok(dto::ConversationPage {
                items: summaries(context, &page.items)?,
                next_cursor: page.next_cursor,
            })
        })
        .await
}

/// Each character's newest one-to-one chat, archived included, so the chats
/// screen can show a character unless its newest chat is archived.
pub async fn conversations_latest_by_character(
    context: &ApiContext,
    request: dto::LatestConversationsRequest,
) -> Result<dto::LatestConversationPage, ApiError> {
    latest_by_source(context, request, ConversationKindTag::Direct).await
}

/// Each group's newest chat, archived included.
pub async fn conversations_latest_by_group(
    context: &ApiContext,
    request: dto::LatestConversationsRequest,
) -> Result<dto::LatestConversationPage, ApiError> {
    latest_by_source(context, request, ConversationKindTag::Group).await
}

async fn latest_by_source(
    context: &ApiContext,
    request: dto::LatestConversationsRequest,
    kind: ConversationKindTag,
) -> Result<dto::LatestConversationPage, ApiError> {
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let page = page_request(request.cursor, request.limit);
            let page = match kind {
                ConversationKindTag::Direct => {
                    ConversationOverviewReader::latest_per_character(database, &page)
                }
                ConversationKindTag::Group => {
                    ConversationOverviewReader::latest_per_group(database, &page)
                }
            }
            .map_err(|error| cursor_error(error, "cursor"))?;
            let items = summaries(context, &page.items)?
                .into_iter()
                .map(|conversation| {
                    let source_id = match &conversation.source {
                        dto::ConversationSource::Direct { character_id } => character_id.clone(),
                        dto::ConversationSource::Group { group_id } => group_id.clone(),
                    };
                    dto::LatestConversation {
                        source_id,
                        conversation,
                    }
                })
                .collect();
            Ok(dto::LatestConversationPage {
                items,
                next_cursor: page.next_cursor,
            })
        })
        .await
}

fn cursor_error(error: ConversationRepositoryError, field: &str) -> ApiError {
    match error {
        ConversationRepositoryError::Invalid(ValidationError::InvalidValue {
            field: "page.cursor",
        }) => invalid_field(field, "the cursor is not valid for this list"),
        error => error.into_api_error(),
    }
}

pub async fn conversation_open(
    context: &ApiContext,
    request: dto::ConversationOpenRequest,
) -> Result<dto::ConversationView, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let aggregate = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            let conversation = aggregate.conversation;
            let branch = aggregate
                .branches
                .iter()
                .find(|branch| branch.id == conversation.active_branch_id)
                .ok_or_else(|| {
                    api_error(
                        ApiErrorCode::Internal,
                        "the conversation's active branch is missing",
                    )
                })?;
            let mut avatars = AvatarLookup::default();
            let participants = conversation
                .participants
                .iter()
                .map(|participant| avatars.participant(context, participant))
                .collect::<Result<Vec<_>, _>>()
                .map_err(IntoApiError::into_api_error)?;
            let messages = message_page(context, conversation_id, branch.id, None, None)?;
            let pending_turn = ConversationOverviewReader::live_turn(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            let can_send = can_send(
                conversation.lifecycle,
                pending_turn.is_some(),
                conversation
                    .participants
                    .iter()
                    .any(|participant| participant.role == ParticipantRole::User),
            );
            let missing_models = MissingModels::new(context)?.of(context, &conversation)?;
            Ok(dto::ConversationView {
                id: conversation.id.to_string(),
                kind: mapping::conversation_kind(&conversation.kind),
                participants,
                branch: dto::BranchHead {
                    branch_id: branch.id.to_string(),
                    head_message_id: branch
                        .head_message_id
                        .or(branch.fork_message_id)
                        .map(|id| id.to_string()),
                },
                messages,
                pending_turn_id: pending_turn.map(|id| id.to_string()),
                can_send,
                revision: conversation.revision.get(),
                settings_revision: conversation
                    .current_settings
                    .as_ref()
                    .map(|settings| settings.revision.get()),
                archived: conversation.lifecycle == ConversationLifecycle::Archived,
                source: mapping::conversation_source(&conversation.kind),
                chat_mode: chat_mode(context, &conversation)?,
                missing_models,
                memory_blocked: super::messages::memory_blocked(context, conversation.id)?,
                title: conversation.title,
            })
        })
        .await
}

/// A chat takes a send unless it is tombstoned (sync can leave one behind),
/// a turn is still unsettled, or it has no user participant; an archived
/// chat stays usable.
pub(super) const fn can_send(
    lifecycle: ConversationLifecycle,
    pending_turn: bool,
    has_user: bool,
) -> bool {
    !matches!(lifecycle, ConversationLifecycle::Tombstoned) && !pending_turn && has_user
}

/// A page of the selected branch's visible messages: the newest, the ones
/// before `before_cursor` or the ones after `after_cursor`.
pub async fn conversation_messages(
    context: &ApiContext,
    request: dto::ConversationMessagesRequest,
) -> Result<dto::MessagePage, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    if request.before_cursor.is_some() && request.after_cursor.is_some() {
        return Err(invalid_field(
            "after_cursor",
            "a page reads before or after a cursor, not both",
        ));
    }
    context
        .blocking(move |context| {
            let aggregate = ConversationReader::get(context.backend().database(), conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            if let Some(cursor) = request.after_cursor {
                return super::messages::newer_page(
                    context,
                    conversation_id,
                    aggregate.conversation.active_branch_id,
                    cursor,
                    request.limit,
                );
            }
            message_page(
                context,
                conversation_id,
                aggregate.conversation.active_branch_id,
                request.before_cursor,
                request.limit,
            )
        })
        .await
}

fn message_page(
    context: &ApiContext,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    cursor: Option<String>,
    limit: Option<u32>,
) -> Result<dto::MessagePage, ApiError> {
    let database = context.backend().database();
    let has_cursor = cursor.is_some();
    let page = ConversationReader::timeline_page(
        database,
        conversation_id,
        branch_id,
        &page_request(cursor, limit),
    )
    .map_err(|error| match error {
        ConversationRepositoryError::Invalid(ValidationError::InvalidValue {
            field: "page.cursor",
        }) if has_cursor => invalid_field("before_cursor", "the message cursor is not valid here"),
        ConversationRepositoryError::Invalid(ValidationError::InvalidReference {
            field: "timeline_page.selected_branch",
        }) => api_error(
            ApiErrorCode::Conflict,
            "the conversation's selected branch is no longer active",
        ),
        error => error.into_api_error(),
    })?;
    let visible = page
        .items
        .iter()
        .filter(|item| item.message.visibility == MessageVisibility::Visible)
        .collect::<Vec<_>>();
    let with_candidates = visible
        .iter()
        .filter(|item| item.message.role == MessageRole::Assistant)
        .map(|item| item.message.id)
        .collect::<Vec<_>>();
    let counts =
        ConversationOverviewReader::candidate_counts(database, conversation_id, &with_candidates)
            .map_err(IntoApiError::into_api_error)?
            .into_iter()
            .collect::<HashMap<_, _>>();
    Ok(dto::MessagePage {
        items: visible
            .iter()
            .rev()
            .map(|item| mapping::timeline_message(context, item, &counts))
            .collect(),
        next_cursor: page.next_cursor,
    })
}

fn send_digest(conversation_id: ConversationId, text: &str) -> Result<ContentHash, ApiError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"lettuce-api-conversation-send-v1\0");
    hasher.update(conversation_id.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(text.as_bytes());
    ContentHash::parse(hasher.finalize().to_hex().as_str())
        .map_err(|_| api_error(ApiErrorCode::Internal, "send digest is not a content hash"))
}

/// Commits the user message and queues its reply; the generation worker
/// streams the reply into `events`. A direct companion chat goes through the
/// companion coordinator.
pub async fn conversation_send(
    context: &ApiContext,
    request: dto::ConversationSendRequest,
    events: Arc<dyn GenerationEventSink>,
) -> Result<dto::SendAccepted, ApiError> {
    send_with(context, request, events, |context, begun, now| {
        context
            .backend()
            .conversation_generation_dispatcher()
            .schedule(begun, now)
    })
    .await
}

pub(super) async fn send_with<S>(
    context: &ApiContext,
    request: dto::ConversationSendRequest,
    events: Arc<dyn GenerationEventSink>,
    schedule: S,
) -> Result<dto::SendAccepted, ApiError>
where
    S: FnOnce(
            &ApiContext,
            &BeginGeneration,
            TimestampMillis,
        )
            -> Result<ConversationGenerationAdmission, ConversationGenerationDispatchError>
        + Send
        + 'static,
{
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    if request.text.trim().is_empty() {
        return Err(invalid_field("text", "the message text is blank"));
    }
    let key = operation_key(request.client_operation_id)?;
    let text = request.text;
    let replay = {
        let operation = OperationToken {
            key: key.clone(),
            request_digest: send_digest(conversation_id, &text)?,
        };
        context
            .blocking(move |context| {
                let record = ConversationReader::operation_record(
                    context.backend().database(),
                    conversation_id,
                    OperationKind::Send,
                    &operation,
                )
                .map_err(IntoApiError::into_api_error)?;
                match record {
                    Some(record) if record.operation.request_digest != operation.request_digest => {
                        Err(api_error(
                            ApiErrorCode::Conflict,
                            "client_operation_id was already used for a different send",
                        ))
                    }
                    Some(_) => Ok(true),
                    None => Ok(false),
                }
            })
            .await?
    };
    if !replay {
        super::turns::preflight(context, conversation_id, TurnOperation::Send).await?;
    }
    let accepted = context
        .blocking(move |context| {
            let database = context.backend().database();
            let now = context.now();
            if !replay {
                crate::conversation::ensure_group_members(database, conversation_id, now)
                    .map_err(super::conversation_settings::edit_error)?;
            }
            let conversation = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?
                .conversation;
            let user = conversation
                .participants
                .iter()
                .find(|participant| participant.role == ParticipantRole::User)
                .ok_or_else(|| {
                    api_error(
                        ApiErrorCode::Unsupported,
                        "the conversation has no user participant",
                    )
                })?;
            let operation = OperationToken {
                key,
                request_digest: send_digest(conversation_id, &text)?,
            };
            if let Some(record) = ConversationReader::operation_record(
                database,
                conversation_id,
                OperationKind::Send,
                &operation,
            )
            .map_err(IntoApiError::into_api_error)?
                && record.operation.request_digest != operation.request_digest
            {
                return Err(api_error(
                    ApiErrorCode::Conflict,
                    "client_operation_id was already used for a different send",
                ));
            }
            let command = SendConversation {
                conversation_id,
                branch_id: conversation.active_branch_id,
                expected_revision: conversation.revision,
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
            };
            let emotion = context.emotion();
            let begun = match CompanionTurnCoordinator::new(database, Some(emotion.as_ref()))
                .begin_send(&command, now, &CancellationToken::new())
            {
                Ok(begun) => begun.value,
                Err(CompanionTurnError::Conversation(
                    ConversationRepositoryError::Conflict
                    | ConversationRepositoryError::StaleRevision { .. },
                )) if ConversationOverviewReader::live_turn(database, conversation_id)
                    .map_err(IntoApiError::into_api_error)?
                    .is_some() =>
                {
                    return Err(api_error(
                        ApiErrorCode::Busy,
                        "the conversation is still generating a reply",
                    ));
                }
                Err(error) => return Err(error.into_api_error()),
            };
            let GenerationInput::UserMessage { message_id } = begun.turn.input else {
                return Err(api_error(
                    ApiErrorCode::Internal,
                    "a send did not start from a user message",
                ));
            };
            let accepted = dto::SendAccepted {
                user_message_id: message_id.to_string(),
                turn_id: begun.turn.id.to_string(),
            };
            schedule_begun(context, &begun, events, schedule, now)?;
            Ok(accepted)
        })
        .await?;
    context.wake_workers();
    Ok(accepted)
}

/// Queues the reply of a begun turn and attaches its stream: a turn that
/// already settled (a replayed request) sends its last event at once.
pub(super) fn schedule_begun<S>(
    context: &ApiContext,
    begun: &BeginGeneration,
    events: Arc<dyn GenerationEventSink>,
    schedule: S,
    now: TimestampMillis,
) -> Result<(), ApiError>
where
    S: FnOnce(
        &ApiContext,
        &BeginGeneration,
        TimestampMillis,
    ) -> Result<ConversationGenerationAdmission, ConversationGenerationDispatchError>,
{
    let database = context.backend().database();
    let turn_id = begun.turn.id;
    if let Some(event) = settled_event(database, turn_id)? {
        events.emit(event);
        return Ok(());
    }
    context.attach_stream(turn_id, events);
    match schedule(context, begun, now) {
        Ok(admission) if admission.job.state.is_terminal() => {
            if let Some(event) = settled_event(database, turn_id)? {
                context.finish_stream(turn_id, event);
            }
        }
        Ok(_) => {}
        Err(error) => {
            unschedulable_turn(context, begun.conversation.id, turn_id);
            return Err(error.into_api_error());
        }
    }
    Ok(())
}

/// Settles a committed turn whose reply could not be queued, so the
/// conversation accepts the next request.
fn unschedulable_turn(
    context: &ApiContext,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
) {
    let database = context.backend().database();
    let settled = ConversationReader::get_turn(database, turn_id)
        .map_err(|error| error.to_string())
        .and_then(|turn| {
            context
                .backend()
                .conversation_generation_dispatcher()
                .settle_unrunnable_turn(&turn, context.now())
                .map_err(|error| error.to_string())
        });
    if let Err(error) = settled {
        tracing::error!(%error, %turn_id, "a turn whose reply could not be queued was not settled");
    }
    let event = match settled_event(database, turn_id) {
        Ok(Some(event)) => event,
        Ok(None) => dto::GenerationEvent::Failed {
            turn_id: turn_id.to_string(),
            code: dto::GenerationFailureCode::Internal,
        },
        Err(error) => {
            tracing::error!(code = ?error.code, message = %error.message, %turn_id, "a turn whose reply could not be queued has no readable outcome");
            dto::GenerationEvent::Failed {
                turn_id: turn_id.to_string(),
                code: dto::GenerationFailureCode::Internal,
            }
        }
    };
    context.settle_turn(conversation_id, turn_id, event);
}

/// Stops a turn. A queued turn is settled here; a running one is signalled
/// and settled by the worker running it, which keeps the reply streamed so
/// far. A turn that is unknown, has no job or already settled is left
/// alone and the call succeeds.
pub async fn generation_cancel(
    context: &ApiContext,
    request: dto::GenerationCancelRequest,
) -> Result<(), ApiError> {
    let turn_id: GenerationTurnId = parse_id(&request.turn_id, "turn_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let turn = match ConversationReader::get_turn(database, turn_id) {
                Ok(turn) => turn,
                Err(ConversationRepositoryError::NotFound) => return Ok(()),
                Err(error) => return Err(error.into_api_error()),
            };
            let Some(job_id) = turn
                .attempts
                .iter()
                .filter(|attempt| attempt.job_id.is_some())
                .max_by_key(|attempt| attempt.ordinal)
                .and_then(|attempt| attempt.job_id)
            else {
                return Ok(());
            };
            match context
                .backend()
                .conversation_generation_cancellation()
                .cancel(job_id, CancellationReason::User, context.now())
                .map_err(IntoApiError::into_api_error)?
            {
                ConversationGenerationCancellationOutcome::QueuedCancelled(_) => {
                    context.settle_turn(
                        turn.conversation_id,
                        turn_id,
                        dto::GenerationEvent::Cancelled {
                            turn_id: turn_id.to_string(),
                        },
                    );
                    Ok(())
                }
                ConversationGenerationCancellationOutcome::Requested { .. }
                | ConversationGenerationCancellationOutcome::AlreadyTerminal(_) => Ok(()),
                ConversationGenerationCancellationOutcome::NotFound => Err(api_error(
                    ApiErrorCode::NotFound,
                    "the turn's generation job was not found",
                )),
            }
        })
        .await
}

/// Starts a one-to-one chat with a character: titled `title` (trimmed) or
/// the character's name, with the chosen scene or the character's default
/// scene, the chosen chat template if any, and the default persona. A scene
/// or template of another character is `InvalidInput` naming the field.
/// Repeating the call with the same key returns the same conversation.
pub async fn conversation_launch_direct(
    context: &ApiContext,
    request: dto::LaunchDirectRequest,
) -> Result<dto::LaunchDirectResponse, ApiError> {
    let character_id: CharacterId = parse_id(&request.character_id, "character_id")?;
    let scene = match request.scene_id.as_deref() {
        Some(id) => LaunchSelection::Explicit(parse_id::<SceneId>(id, "scene_id")?),
        None => LaunchSelection::Inherit,
    };
    let starter = match request.starter_id.as_deref() {
        Some(id) => LaunchSelection::Explicit(parse_id::<ConversationStarterId>(id, "starter_id")?),
        None => LaunchSelection::Inherit,
    };
    let title = request
        .title
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty());
    let operation_key = operation_key(request.client_operation_id)?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let character = CharacterRepository::get(database, character_id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the character was not found"))?;
            let launch = DirectConversationLaunchRequest {
                format_version: DIRECT_LAUNCH_REQUEST_FORMAT_V1,
                title: title.unwrap_or_else(|| character.character.profile.name.clone()),
                user: DirectUserParticipant {
                    display_name: DEFAULT_USER_DISPLAY_NAME.to_owned(),
                    authored_description: None,
                },
                character_id,
                scene,
                starter,
                persona: LaunchSelection::Inherit,
                operation_key,
            };
            let conversation_id = match context
                .backend()
                .launch_direct_conversation(&launch, context.now())
            {
                Ok(created) => created.value.conversation.id,
                Err(ConversationLaunchError::AlreadyLaunched { conversation_id }) => {
                    conversation_id
                }
                Err(error) => return Err(error.into_api_error()),
            };
            Ok(dto::LaunchDirectResponse {
                conversation_id: conversation_id.to_string(),
            })
        })
        .await
}

/// Starts a chat from a group profile, titled with the group's name and
/// taking its members, settings and persona. Repeating the call with the
/// same key returns the same conversation.
pub async fn conversation_launch_group(
    context: &ApiContext,
    request: dto::LaunchGroupRequest,
) -> Result<dto::LaunchGroupResponse, ApiError> {
    let group_id: GroupId = parse_id(&request.group_id, "group_id")?;
    let operation_key = operation_key(request.client_operation_id)?;
    context
        .blocking(move |context| {
            let launch = GroupConversationLaunchRequest {
                format_version: GROUP_LAUNCH_REQUEST_FORMAT_V1,
                title: String::new(),
                user: DirectUserParticipant {
                    display_name: DEFAULT_USER_DISPLAY_NAME.to_owned(),
                    authored_description: None,
                },
                group_id,
                persona: LaunchSelection::Inherit,
                operation_key,
            };
            let conversation_id = match context
                .backend()
                .launch_group_conversation(&launch, context.now())
            {
                Ok(created) => created.value.conversation.id,
                Err(ConversationLaunchError::AlreadyLaunched { conversation_id }) => {
                    conversation_id
                }
                Err(error) => return Err(error.into_api_error()),
            };
            Ok(dto::LaunchGroupResponse {
                conversation_id: conversation_id.to_string(),
            })
        })
        .await
}
