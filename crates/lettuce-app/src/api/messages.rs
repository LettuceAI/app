//! Message operations and the timeline reads around them: edit, delete,
//! delete-after, pin, variant and scene choice, histories, search, pinned
//! messages, the count and pages anchored at a message.

use std::collections::HashMap;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    ChooseCandidate, ConversationKind, ConversationOverviewReader, ConversationReader,
    ConversationRepository, ConversationRepositoryError, DescendantPolicy, EditMessage,
    MessageEditDraft, MessagePart, MessageRenderSource, MessageRole, MessageVisibility,
    OperationKind, OperationToken, PreparedSceneSelection, TimelineItem, TimelinePage,
    TombstoneMessage, UpdateMessageFlags, ValidationError,
};
use lettuce_types::{
    AssetId, ConversationBranchId, ConversationId, MessageCandidateId, MessageId, PageLimit,
    PageRequest, Revision, SceneId,
};

use super::ApiContext;
use super::conversation_delete::{DELETE_SETTLE_LIMIT, cancel_memory_work};
use super::conversation_settings::edit_error;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::mapping;
use crate::conversation;

/// The timeline page a search or pinned list reads at a time.
const SCAN_PAGE: u16 = 200;

pub(super) fn expected_revision(value: u64) -> Result<Revision, ApiError> {
    if value == 0 {
        return Err(invalid_field(
            "expected_revision",
            "a revision starts at one",
        ));
    }
    Ok(Revision::new(value))
}

pub(super) fn operation(key: String, parts: &[&[u8]]) -> Result<OperationToken, ApiError> {
    conversation::edit_operation(key, parts).map_err(edit_error)
}

fn cursor_field(error: ConversationRepositoryError, field: &str) -> ApiError {
    match error {
        ConversationRepositoryError::Invalid(ValidationError::InvalidValue {
            field: "page.cursor",
        }) => invalid_field(field, "the cursor is not valid here"),
        ConversationRepositoryError::Invalid(ValidationError::InvalidReference {
            field: "timeline_page.selected_branch",
        }) => api_error(
            ApiErrorCode::Conflict,
            "the conversation's selected branch is no longer active",
        ),
        error => error.into_api_error(),
    }
}

pub(super) fn active_branch(
    context: &ApiContext,
    conversation_id: ConversationId,
) -> Result<ConversationBranchId, ApiError> {
    Ok(
        ConversationReader::get(context.backend().database(), conversation_id)
            .map_err(IntoApiError::into_api_error)?
            .conversation
            .active_branch_id,
    )
}

/// The message as it stands on the active branch, whatever its visibility.
pub(super) fn on_timeline(
    context: &ApiContext,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    message_id: MessageId,
) -> Result<lettuce_conversations::TimelineAnchor, ApiError> {
    ConversationOverviewReader::timeline_anchor(
        context.backend().database(),
        conversation_id,
        branch_id,
        message_id,
    )
    .map_err(|error| match error {
        ConversationRepositoryError::NotFound => api_error(
            ApiErrorCode::NotFound,
            "the message is not on the conversation's selected branch",
        ),
        error => cursor_field(error, "message_id"),
    })
}

/// Whether `operation` already ran as `kind`; the same key with another
/// request is `Conflict`.
pub(super) fn replayed(
    context: &ApiContext,
    conversation_id: ConversationId,
    kind: OperationKind,
    operation: &OperationToken,
) -> Result<bool, ApiError> {
    match ConversationReader::operation_record(
        context.backend().database(),
        conversation_id,
        kind,
        operation,
    )
    .map_err(IntoApiError::into_api_error)?
    {
        Some(record) if record.operation.request_digest == operation.request_digest => Ok(true),
        Some(_) => Err(api_error(
            ApiErrorCode::Conflict,
            "client_operation_id was already used for a different request",
        )),
        None => Ok(false),
    }
}

fn timeline_messages(
    context: &ApiContext,
    conversation_id: ConversationId,
    items: &[&TimelineItem],
) -> Result<Vec<dto::TimelineMessage>, ApiError> {
    let with_candidates = items
        .iter()
        .filter(|item| item.message.role == MessageRole::Assistant)
        .map(|item| item.message.id)
        .collect::<Vec<_>>();
    let counts = ConversationOverviewReader::candidate_counts(
        context.backend().database(),
        conversation_id,
        &with_candidates,
    )
    .map_err(IntoApiError::into_api_error)?
    .into_iter()
    .collect::<HashMap<_, _>>();
    let scene_images = super::scenes::views(context, conversation_id, items)?;
    Ok(items
        .iter()
        .map(|item| mapping::timeline_message(context, item, &counts, &scene_images))
        .collect())
}

fn visible(page: &TimelinePage) -> impl Iterator<Item = &TimelineItem> {
    page.items
        .iter()
        .filter(|item| item.message.visibility == MessageVisibility::Visible)
}

/// A changed message as the chat shows it now.
pub(super) fn changed(
    context: &ApiContext,
    message: lettuce_conversations::Message,
    revision: u64,
) -> Result<dto::MessageChanged, ApiError> {
    let database = context.backend().database();
    let (active_revision, active_candidate) = match message.active_render_source {
        MessageRenderSource::Revision(id) => (
            Some(
                ConversationReader::get_message_revision(database, id)
                    .map_err(IntoApiError::into_api_error)?,
            ),
            None,
        ),
        MessageRenderSource::Candidate(id) => (
            None,
            Some(
                ConversationReader::get_candidate(database, id)
                    .map_err(IntoApiError::into_api_error)?,
            ),
        ),
    };
    let conversation_id = message.conversation_id;
    let item = TimelineItem {
        message,
        active_revision,
        active_candidate,
        initial_origin: None,
    };
    Ok(dto::MessageChanged {
        message: timeline_messages(context, conversation_id, &[&item])?.remove(0),
        revision,
    })
}

/// The conversation revision a commit left.
pub(super) fn committed_revision(
    outbox: &[lettuce_conversations::ConversationOutboxRecord],
) -> u64 {
    outbox
        .iter()
        .map(|record| record.conversation_revision.get())
        .max()
        .unwrap_or_default()
}

/// The message after a replayed edit: as it stands now, with the current
/// conversation revision.
pub(super) fn current_message(
    context: &ApiContext,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<dto::MessageChanged, ApiError> {
    let conversation = ConversationReader::get(context.backend().database(), conversation_id)
        .map_err(IntoApiError::into_api_error)?
        .conversation;
    let anchor = on_timeline(
        context,
        conversation_id,
        conversation.active_branch_id,
        message_id,
    )?;
    Ok(dto::MessageChanged {
        message: timeline_messages(context, conversation_id, &[&anchor.item])?.remove(0),
        revision: conversation.revision.get(),
    })
}

/// The edited message's parts: `text` in place of its text, the media it
/// keeps, everything else as it was.
fn edited_parts(
    shown: &[MessagePart],
    text: String,
    keep: &[AssetId],
) -> Result<Vec<MessagePart>, ApiError> {
    let mut text = Some(text);
    let mut parts = Vec::with_capacity(shown.len() + 1);
    for part in shown {
        match part {
            MessagePart::Text { .. } => {
                if let Some(text) = text.take() {
                    parts.push(MessagePart::Text { text });
                }
            }
            MessagePart::MediaAsset { asset_id, .. } => {
                if keep.contains(asset_id) {
                    parts.push(part.clone());
                }
            }
            _ => parts.push(part.clone()),
        }
    }
    if let Some(text) = text {
        parts.insert(0, MessagePart::Text { text });
    }
    let shown_media = shown
        .iter()
        .filter_map(|part| match part {
            MessagePart::MediaAsset { asset_id, .. } => Some(*asset_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    if keep.iter().any(|asset_id| !shown_media.contains(asset_id)) {
        return Err(invalid_field(
            "keep_media",
            "keep_media names media the message does not have",
        ));
    }
    Ok(parts)
}

/// Replaces a message's text and drops the attachments `keep_media` does
/// not name. The text is trimmed and must not be blank. A reply edit
/// rewrites its shown variant; a scene edit marks the scene edited. A
/// message a running turn is writing is `Conflict`.
pub async fn message_edit(
    context: &ApiContext,
    request: dto::MessageEditRequest,
) -> Result<dto::MessageChanged, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let text = request.text.trim().to_owned();
    if text.is_empty() {
        return Err(invalid_field("text", "the message text is blank"));
    }
    let mut keep = request
        .keep_media
        .iter()
        .map(|id| parse_id::<AssetId>(id, "keep_media"))
        .collect::<Result<Vec<_>, _>>()?;
    keep.sort_unstable();
    keep.dedup();
    let expected_revision = expected_revision(request.expected_revision)?;
    let kept = keep
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let operation = operation(
        request.client_operation_id,
        &[
            b"lettuce-message-edit-v1",
            conversation_id.to_string().as_bytes(),
            message_id.to_string().as_bytes(),
            text.as_bytes(),
            kept.as_bytes(),
        ],
    )?;
    context
        .blocking(move |context| {
            if replayed(context, conversation_id, OperationKind::Edit, &operation)? {
                return current_message(context, conversation_id, message_id);
            }
            let branch_id = active_branch(context, conversation_id)?;
            let item = on_timeline(context, conversation_id, branch_id, message_id)?.item;
            if item.message.visibility == MessageVisibility::Tombstoned {
                return Err(api_error(ApiErrorCode::NotFound, "the message was deleted"));
            }
            let parts = edited_parts(mapping::shown_parts(&item), text, &keep)?;
            let edited = ConversationRepository::edit_message(
                context.backend().database(),
                &EditMessage {
                    conversation_id,
                    message_id,
                    expected_revision,
                    operation,
                    draft: MessageEditDraft {
                        parts,
                        visibility: item.message.visibility,
                        pinned: item.message.pinned,
                        scene_edited: item.message.role == MessageRole::Scene,
                    },
                },
                context.now(),
            )
            .map_err(|error| match error {
                ConversationRepositoryError::Invalid(ValidationError::TooLarge { .. }) => {
                    invalid_field("text", "the message text is too long")
                }
                error => error.into_api_error(),
            })?;
            let revision = committed_revision(&edited.outbox);
            changed(context, edited.value.message, revision)
        })
        .await
}

/// Deletes one message; the messages after it stay. A message another
/// branch also shows is not deleted from that branch: a new branch without
/// it is forked and selected instead (`Branched`). Deleting a one-to-one
/// chat's scene message turns its scene setting off in the same change.
pub async fn message_delete(
    context: &ApiContext,
    request: dto::MessageDeleteRequest,
) -> Result<dto::MessagesDeleteResult, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let expected_revision = expected_revision(request.expected_revision)?;
    let operation = operation(
        request.client_operation_id,
        &[
            b"lettuce-message-delete-v1",
            conversation_id.to_string().as_bytes(),
            message_id.to_string().as_bytes(),
        ],
    )?;
    context
        .blocking(move |context| {
            let replay = replayed(context, conversation_id, OperationKind::Tombstone, &operation)?;
            if !replay {
                let branch_id = active_branch(context, conversation_id)?;
                on_timeline(context, conversation_id, branch_id, message_id)?;
            }
            let deleted = ConversationRepository::delete_message(
                context.backend().database(),
                &TombstoneMessage {
                    conversation_id,
                    message_id,
                    expected_revision,
                    operation,
                    descendants: DescendantPolicy::Preserve,
                },
                context.now(),
            )
            .map_err(|error| match error {
                ConversationRepositoryError::Unsupported => api_error(
                    ApiErrorCode::Unsupported,
                    "the first message is shown by another branch and cannot be deleted from this one",
                ),
                error => error.into_api_error(),
            })?;
            Ok(match deleted.value {
                lettuce_conversations::DeleteMessageOutcome::Tombstoned(tombstoned) => {
                    dto::MessagesDeleteResult {
                        outcome: dto::MessagesDeleteOutcome::Tombstoned {
                            removed: vec![message_id.to_string()],
                        },
                        revision: tombstoned.conversation.revision.get(),
                    }
                }
                lettuce_conversations::DeleteMessageOutcome::Branched(branch) => {
                    dto::MessagesDeleteResult {
                        outcome: dto::MessagesDeleteOutcome::Branched {
                            branch_id: branch.branch.id.to_string(),
                        },
                        revision: branch.conversation.revision.get(),
                    }
                }
            })
        })
        .await
}

/// Deletes every message after one on the selected branch and rewinds
/// dynamic memory to match; when some of those messages belong to the
/// branch the selected one came from, forks a new branch at the message
/// instead and deletes nothing. A memory cycle still running for the chat is
/// stopped first; the delete waits for it, woken by committed changes. The
/// rewind the delete owes is recorded with it, so one a crash interrupts
/// finishes at startup or before the chat's next memory cycle.
pub async fn messages_delete_after(
    context: &ApiContext,
    request: dto::MessageDeleteRequest,
) -> Result<dto::MessagesDeleteResult, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let expected_revision = expected_revision(request.expected_revision)?;
    let operation = operation(
        request.client_operation_id,
        &[
            b"lettuce-messages-delete-after-v1",
            conversation_id.to_string().as_bytes(),
            message_id.to_string().as_bytes(),
        ],
    )?;
    let replay = {
        let operation = operation.clone();
        context
            .blocking(move |context| {
                Ok(replayed(
                    context,
                    conversation_id,
                    OperationKind::Tombstone,
                    &operation,
                )? || replayed(context, conversation_id, OperationKind::Fork, &operation)?)
            })
            .await?
    };
    if !replay {
        stop_memory_work(context, conversation_id).await?;
    }
    let deleted = context
        .blocking(move |context| {
            let database = context.backend().database();
            let summary_message_interval = summary_message_interval(context, conversation_id)?;
            crate::DynamicMemoryDeleteAfterCoordinator::new(database, database)
                .delete_after(
                    &crate::DeleteAfterMessages {
                        conversation_id,
                        after_message_id: message_id,
                        expected_revision,
                        operation,
                        summary_message_interval,
                    },
                    context.now(),
                )
                .map_err(|error| delete_after_error(context, conversation_id, error))
        })
        .await?;
    context.jobs().wake();
    let outcome = match deleted.branch_id {
        Some(branch_id) => dto::MessagesDeleteOutcome::Branched {
            branch_id: branch_id.to_string(),
        },
        None => {
            let mut removed = Vec::new();
            if let Some(tombstone) = &deleted.tombstone {
                removed.push(tombstone.value.message.id.to_string());
                for record in &tombstone.outbox {
                    if let lettuce_conversations::ConversationOutboxEvent::MessageTombstoned {
                        affected_message_ids,
                        ..
                    } = &record.event
                    {
                        removed.extend(affected_message_ids.iter().map(ToString::to_string));
                    }
                }
            }
            dto::MessagesDeleteOutcome::Tombstoned { removed }
        }
    };
    Ok(dto::MessagesDeleteResult {
        outcome,
        revision: deleted.conversation.revision.get(),
    })
}

/// Cancels the conversation's memory cycles and waits until none runs.
async fn stop_memory_work(
    context: &ApiContext,
    conversation_id: ConversationId,
) -> Result<(), ApiError> {
    let mut changes = context.committed_changes();
    let deadline = tokio::time::Instant::now() + DELETE_SETTLE_LIMIT;
    loop {
        changes.borrow_and_update();
        let unfinished = context
            .blocking(move |context| cancel_memory_work(context, conversation_id))
            .await?;
        if unfinished == 0 {
            return Ok(());
        }
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    return Err(api_error(ApiErrorCode::Internal, "the change signal closed"));
                }
            }
            () = tokio::time::sleep_until(deadline) => {
                return Err(api_error(
                    ApiErrorCode::Busy,
                    "the conversation's memory cycle did not stop",
                ));
            }
        }
    }
}

/// The summary interval the chat's dynamic memory rebuilds with: the group
/// or the one-to-one setting.
fn summary_message_interval(
    context: &ApiContext,
    conversation_id: ConversationId,
) -> Result<u32, ApiError> {
    let database = context.backend().database();
    let conversation = ConversationReader::get(database, conversation_id)
        .map_err(IntoApiError::into_api_error)?
        .conversation;
    let settings = lettuce_settings::GlobalSettingsStore::load(database)
        .map_err(|_| api_error(ApiErrorCode::Internal, "the app settings could not be read"))?
        .settings;
    Ok(match conversation.kind {
        ConversationKind::Group(_) => settings.effective_group_dynamic_memory(),
        ConversationKind::Direct(_) => &settings.dynamic_memory,
    }
    .summary_message_interval)
}

pub(super) fn delete_after_error(
    context: &ApiContext,
    conversation_id: ConversationId,
    error: crate::DynamicMemoryDeleteAfterError,
) -> ApiError {
    match error {
        crate::DynamicMemoryDeleteAfterError::Conversation(
            ConversationRepositoryError::Conflict,
        ) if ConversationOverviewReader::live_turn(
            context.backend().database(),
            conversation_id,
        )
        .is_ok_and(|turn| turn.is_some()) =>
        {
            api_error(
                ApiErrorCode::Busy,
                "the conversation is still generating a reply",
            )
        }
        crate::DynamicMemoryDeleteAfterError::OwedRewind {
            conversation_id,
            reason,
        } => ApiError {
            code: ApiErrorCode::Unavailable,
            message: format!("a delete's memory rewind is still owed and failed: {reason}"),
            details: Some(dto::ApiErrorDetails::PendingMemoryRewind {
                conversation_id: conversation_id.to_string(),
            }),
        },
        crate::DynamicMemoryDeleteAfterError::Conversation(error) => error.into_api_error(),
        crate::DynamicMemoryDeleteAfterError::Rewind(
            lettuce_memory::DynamicMemorySuffixRewindError::Conflict,
        ) => api_error(
            ApiErrorCode::Conflict,
            "memory changed while it was rewound; repeat the request",
        ),
        error => api_error(ApiErrorCode::Internal, error.to_string()),
    }
}

/// Pins or unpins a message; setting the state it already has changes
/// nothing else.
pub async fn message_pin(
    context: &ApiContext,
    request: dto::MessagePinRequest,
) -> Result<dto::MessageChanged, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let expected_revision = expected_revision(request.expected_revision)?;
    let pinned = request.pinned;
    let operation = operation(
        request.client_operation_id,
        &[
            b"lettuce-message-pin-v1",
            conversation_id.to_string().as_bytes(),
            message_id.to_string().as_bytes(),
            &[u8::from(pinned)],
        ],
    )?;
    context
        .blocking(move |context| {
            let flagged = ConversationRepository::update_message_flags(
                context.backend().database(),
                &UpdateMessageFlags {
                    conversation_id,
                    message_id,
                    expected_revision,
                    operation,
                    pinned: Some(pinned),
                    visibility: None,
                },
                context.now(),
            )
            .map_err(IntoApiError::into_api_error)?;
            let revision = committed_revision(&flagged.outbox);
            changed(context, flagged.value, revision)
        })
        .await
}

/// Shows another generated variant of a reply.
pub async fn message_candidate_select(
    context: &ApiContext,
    request: dto::MessageCandidateSelectRequest,
) -> Result<dto::MessageChanged, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let candidate_id: MessageCandidateId = parse_id(&request.candidate_id, "candidate_id")?;
    let expected_revision = expected_revision(request.expected_revision)?;
    let operation = operation(
        request.client_operation_id,
        &[
            b"lettuce-message-candidate-select-v1",
            conversation_id.to_string().as_bytes(),
            message_id.to_string().as_bytes(),
            candidate_id.to_string().as_bytes(),
        ],
    )?;
    context
        .blocking(move |context| {
            let chosen = ConversationRepository::choose_candidate(
                context.backend().database(),
                &ChooseCandidate {
                    conversation_id,
                    message_id,
                    candidate_id,
                    expected_revision,
                    operation,
                },
                context.now(),
            )
            .map_err(IntoApiError::into_api_error)?;
            let revision = committed_revision(&chosen.outbox);
            changed(context, chosen.value, revision)
        })
        .await
}

/// Switches a one-to-one chat to another scene of its character from its
/// scene message: the scene setting and the message's text change in one
/// commit, and the message is no longer marked edited.
pub async fn message_scene_select(
    context: &ApiContext,
    request: dto::MessageSceneSelectRequest,
) -> Result<dto::MessageChanged, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let scene_id: SceneId = parse_id(&request.scene_id, "scene_id")?;
    let expected_revision = expected_revision(request.expected_revision)?;
    let operation = operation(
        request.client_operation_id,
        &[
            b"lettuce-message-scene-select-v1",
            conversation_id.to_string().as_bytes(),
            message_id.to_string().as_bytes(),
            scene_id.to_string().as_bytes(),
        ],
    )?;
    context
        .blocking(move |context| {
            if replayed(context, conversation_id, OperationKind::Edit, &operation)? {
                return current_message(context, conversation_id, message_id);
            }
            let database = context.backend().database();
            let conversation = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?
                .conversation;
            if !matches!(conversation.kind, ConversationKind::Direct(_)) {
                return Err(api_error(
                    ApiErrorCode::Unsupported,
                    "only a one-to-one chat switches scenes from its scene message",
                ));
            }
            let item = on_timeline(
                context,
                conversation_id,
                conversation.active_branch_id,
                message_id,
            )?
            .item;
            if item.message.role != MessageRole::Scene
                || item.message.visibility == MessageVisibility::Tombstoned
            {
                return Err(invalid_field(
                    "message_id",
                    "the message is not the chat's scene message",
                ));
            }
            let (settings, text) = conversation::prepare_scene_selection(
                database,
                &conversation,
                scene_id,
                operation.clone(),
            )
            .map_err(edit_error)?;
            let text = text.ok_or_else(|| invalid_field("scene_id", "the scene has no text"))?;
            let selection = PreparedSceneSelection::new(
                EditMessage {
                    conversation_id,
                    message_id,
                    expected_revision,
                    operation,
                    draft: MessageEditDraft {
                        parts: vec![MessagePart::Text { text }],
                        visibility: item.message.visibility,
                        pinned: item.message.pinned,
                        scene_edited: false,
                    },
                },
                settings,
            )
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            let selected = ConversationRepository::select_scene(database, selection, context.now())
                .map_err(IntoApiError::into_api_error)?;
            let revision = committed_revision(&selected.outbox);
            changed(context, selected.value.message, revision)
        })
        .await
}

/// A message's saved versions, oldest first.
pub async fn message_revisions(
    context: &ApiContext,
    request: dto::MessageHistoryRequest,
) -> Result<dto::MessageRevisionPage, ApiError> {
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    context
        .blocking(move |context| {
            let page = ConversationReader::page_message_revisions(
                context.backend().database(),
                message_id,
                &PageRequest {
                    cursor: request.cursor,
                    limit: PageLimit::default(),
                },
            )
            .map_err(|error| cursor_field(error, "cursor"))?;
            Ok(dto::MessageRevisionPage {
                items: page
                    .items
                    .iter()
                    .map(|revision| {
                        let (parts, reasoning) = mapping::part_views(context, &revision.parts);
                        dto::MessageRevisionView {
                            id: revision.id.to_string(),
                            sequence: revision.sequence.get(),
                            parts,
                            reasoning,
                            authored_at: revision.authored_at.get(),
                            supersedes_candidate_id: revision
                                .supersedes_candidate_id
                                .map(|id| id.to_string()),
                        }
                    })
                    .collect(),
                next_cursor: page.next_cursor,
            })
        })
        .await
}

/// A reply's generated variants in order, each as it was generated.
pub async fn message_candidates(
    context: &ApiContext,
    request: dto::MessageHistoryRequest,
) -> Result<dto::MessageCandidatePage, ApiError> {
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    context
        .blocking(move |context| {
            let page = ConversationReader::page_candidates(
                context.backend().database(),
                message_id,
                &PageRequest {
                    cursor: request.cursor,
                    limit: PageLimit::default(),
                },
            )
            .map_err(|error| cursor_field(error, "cursor"))?;
            Ok(dto::MessageCandidatePage {
                items: page
                    .items
                    .iter()
                    .map(|candidate| {
                        let (parts, reasoning) = mapping::part_views(context, &candidate.parts);
                        dto::MessageCandidateView {
                            id: candidate.id.to_string(),
                            index: candidate.ordinal,
                            author_participant_id: Some(
                                candidate.author_participant_id.to_string(),
                            ),
                            parts,
                            reasoning,
                            created_at: candidate.created_at.get(),
                        }
                    })
                    .collect(),
                next_cursor: page.next_cursor,
            })
        })
        .await
}

/// Walks the branch timeline oldest first from `cursor`, one bounded page
/// at a time, and keeps the items `matches` accepts until one more than
/// `limit` is found. The cursor then continues after the last kept item.
fn scan_forward(
    context: &ApiContext,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    cursor: Option<String>,
    limit: usize,
    matches: impl Fn(&TimelineItem) -> bool,
) -> Result<(Vec<TimelineItem>, Option<String>), ApiError> {
    let database = context.backend().database();
    let mut found = Vec::new();
    let mut last_cursor = String::new();
    let mut page = PageRequest {
        cursor,
        limit: PageLimit::new(SCAN_PAGE),
    };
    loop {
        let scan = ConversationOverviewReader::timeline_scan_after(
            database,
            conversation_id,
            branch_id,
            &page,
        )
        .map_err(|error| cursor_field(error, "cursor"))?;
        let timeline = scan.page;
        for (item, item_cursor) in timeline.items.into_iter().zip(scan.item_cursors) {
            if matches(&item) {
                found.push(item);
                if found.len() > limit {
                    found.truncate(limit);
                    return Ok((found, Some(last_cursor)));
                }
                last_cursor = item_cursor;
            }
        }
        match timeline.next_cursor {
            Some(next) => page.cursor = Some(next),
            None => return Ok((found, None)),
        }
    }
}

fn page_size(limit: Option<u32>) -> usize {
    usize::from(mapping::page_limit(limit).get())
}

/// Messages on the selected branch whose shown text contains the query,
/// oldest first: visible user, reply and scene messages, matched on the
/// text they show (not other variants or reasoning), ignoring case the way
/// Unicode lowercases. The query is trimmed; an empty one finds nothing
/// without reading the chat.
pub async fn conversation_search(
    context: &ApiContext,
    request: dto::ConversationSearchRequest,
) -> Result<dto::SearchHitPage, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let query = request.query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(dto::SearchHitPage {
            items: Vec::new(),
            next_cursor: None,
        });
    }
    let limit = page_size(request.limit);
    context
        .blocking(move |context| {
            let branch_id = active_branch(context, conversation_id)?;
            let (hits, next_cursor) = scan_forward(
                context,
                conversation_id,
                branch_id,
                request.cursor,
                limit,
                |item| {
                    item.message.visibility == MessageVisibility::Visible
                        && matches!(
                            item.message.role,
                            MessageRole::User | MessageRole::Assistant | MessageRole::Scene
                        )
                        && mapping::shown_text(item)
                            .is_some_and(|text| text.to_lowercase().contains(&query))
                },
            )?;
            Ok(dto::SearchHitPage {
                items: hits
                    .iter()
                    .map(|item| dto::SearchHit {
                        message_id: item.message.id.to_string(),
                        role: mapping::message_role(item.message.role),
                        author_participant_id: item
                            .message
                            .author_participant_id
                            .map(|id| id.to_string()),
                        text: mapping::shown_text(item).unwrap_or_default(),
                        created_at: item.message.created_at.get(),
                    })
                    .collect(),
                next_cursor,
            })
        })
        .await
}

/// The pinned visible messages of the selected branch, oldest first.
pub async fn conversation_pinned_messages(
    context: &ApiContext,
    request: dto::ConversationPinnedMessagesRequest,
) -> Result<dto::MessagePage, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let limit = page_size(request.limit);
    context
        .blocking(move |context| {
            let branch_id = active_branch(context, conversation_id)?;
            let (pinned, next_cursor) = scan_forward(
                context,
                conversation_id,
                branch_id,
                request.cursor,
                limit,
                |item| item.message.pinned && item.message.visibility == MessageVisibility::Visible,
            )?;
            Ok(dto::MessagePage {
                items: timeline_messages(
                    context,
                    conversation_id,
                    &pinned.iter().collect::<Vec<_>>(),
                )?,
                next_cursor,
            })
        })
        .await
}

/// Visible messages on the selected branch, system notes excluded, as the
/// conversation list counts them.
pub async fn conversation_message_count(
    context: &ApiContext,
    request: dto::ConversationRequest,
) -> Result<dto::MessageCount, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| {
            Ok(dto::MessageCount {
                count: ConversationOverviewReader::message_count(
                    context.backend().database(),
                    conversation_id,
                )
                .map_err(IntoApiError::into_api_error)?,
            })
        })
        .await
}

/// A newer page of the selected branch after an `after_cursor`, oldest
/// first.
pub(super) fn newer_page(
    context: &ApiContext,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    cursor: String,
    limit: Option<u32>,
) -> Result<dto::MessagePage, ApiError> {
    let page = ConversationOverviewReader::timeline_page_after(
        context.backend().database(),
        conversation_id,
        branch_id,
        &PageRequest {
            cursor: Some(cursor),
            limit: mapping::page_limit(limit),
        },
    )
    .map_err(|error| cursor_field(error, "after_cursor"))?;
    Ok(dto::MessagePage {
        items: timeline_messages(
            context,
            conversation_id,
            &visible(&page).collect::<Vec<_>>(),
        )?,
        next_cursor: page.next_cursor,
    })
}

/// The visible messages around one message of the selected branch, for
/// jumping to it. A message off the branch, or deleted, is `NotFound`.
pub async fn conversation_messages_around(
    context: &ApiContext,
    request: dto::ConversationMessagesAroundRequest,
) -> Result<dto::MessageWindow, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let branch_id = active_branch(context, conversation_id)?;
            let anchor = on_timeline(context, conversation_id, branch_id, message_id)?;
            if anchor.item.message.visibility != MessageVisibility::Visible {
                return Err(api_error(
                    ApiErrorCode::NotFound,
                    "the message is not shown in the conversation",
                ));
            }
            let (older, before_cursor) = match anchor.older_cursor.clone() {
                Some(cursor) if request.before > 0 => {
                    let page = ConversationReader::timeline_page(
                        database,
                        conversation_id,
                        branch_id,
                        &PageRequest {
                            cursor: Some(cursor),
                            limit: mapping::page_limit(Some(request.before)),
                        },
                    )
                    .map_err(|error| cursor_field(error, "message_id"))?;
                    let next = page.next_cursor.clone();
                    (Some(page), next)
                }
                cursor => (None, cursor),
            };
            let (newer, after_cursor) = match anchor.newer_cursor.clone() {
                Some(cursor) if request.after > 0 => {
                    let page = ConversationOverviewReader::timeline_page_after(
                        database,
                        conversation_id,
                        branch_id,
                        &PageRequest {
                            cursor: Some(cursor),
                            limit: mapping::page_limit(Some(request.after)),
                        },
                    )
                    .map_err(|error| cursor_field(error, "message_id"))?;
                    let next = page.next_cursor.clone();
                    (Some(page), next)
                }
                cursor => (None, cursor),
            };
            let mut items = older
                .as_ref()
                .map(|page| visible(page).collect::<Vec<_>>())
                .unwrap_or_default();
            items.reverse();
            items.push(&anchor.item);
            if let Some(page) = newer.as_ref() {
                items.extend(visible(page));
            }
            Ok(dto::MessageWindow {
                items: timeline_messages(context, conversation_id, &items)?,
                before_cursor,
                after_cursor,
            })
        })
        .await
}

fn failure_code(failure: lettuce_memory::OwedRewindFailure) -> dto::MemoryRewindFailureCode {
    match failure {
        lettuce_memory::OwedRewindFailure::Conflict => dto::MemoryRewindFailureCode::Conflict,
        lettuce_memory::OwedRewindFailure::Inconsistent => {
            dto::MemoryRewindFailureCode::Inconsistent
        }
        lettuce_memory::OwedRewindFailure::Storage => dto::MemoryRewindFailureCode::Storage,
        lettuce_memory::OwedRewindFailure::Other => dto::MemoryRewindFailureCode::Other,
    }
}

/// Why the conversation's memory is stopped: a rewind a delete owes failed.
pub(super) fn memory_blocked(
    context: &ApiContext,
    conversation_id: ConversationId,
) -> Result<Option<dto::MemoryBlockedReason>, ApiError> {
    Ok(
        lettuce_memory::PendingSuffixRewindRepository::pending_rewind_failure(
            context.backend().database(),
            conversation_id,
        )
        .map_err(|_| {
            api_error(
                ApiErrorCode::Internal,
                "the owed memory rewinds could not be read",
            )
        })?
        .map(|failure| dto::MemoryBlockedReason::OwedRewindFailed {
            code: failure_code(failure),
        }),
    )
}

/// Retries the memory rewind a delete owes the chat now. A rewind that
/// still fails stays recorded and is reported; nothing skips or discards it.
pub async fn memory_rewind_retry(
    context: &ApiContext,
    request: dto::MemoryRewindRetryRequest,
) -> Result<dto::MemoryRewindRetryOutcome, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let outcome = context
        .blocking(move |context| {
            let database = context.backend().database();
            ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            let report = crate::DynamicMemoryDeleteAfterCoordinator::new(database, database)
                .complete_pending(Some(conversation_id), context.now())
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            Ok(if report.failed.is_empty() {
                if report.completed == 0 {
                    dto::MemoryRewindRetryOutcome::NothingOwed
                } else {
                    dto::MemoryRewindRetryOutcome::Completed
                }
            } else {
                match memory_blocked(context, conversation_id)? {
                    Some(dto::MemoryBlockedReason::OwedRewindFailed { code }) => {
                        dto::MemoryRewindRetryOutcome::StillFailing { code }
                    }
                    None => dto::MemoryRewindRetryOutcome::Completed,
                }
            })
        })
        .await?;
    context.jobs().wake();
    Ok(outcome)
}
