use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::PurgeError;

use super::ApiContext;
use super::conversation_delete::{BeforeSettle, DELETE_SETTLE_LIMIT, cancel_work};
use super::error::{api_error, parse_id};

fn failure(error: PurgeError) -> ApiError {
    api_error(
        match error {
            PurgeError::Conflict => ApiErrorCode::Conflict,
            PurgeError::NotFound => ApiErrorCode::NotFound,
            PurgeError::Busy => ApiErrorCode::Busy,
            PurgeError::Integrity | PurgeError::Storage => ApiErrorCode::Internal,
        },
        error.to_string(),
    )
}

pub async fn group_delete(
    context: &ApiContext,
    request: dto::GroupDeleteRequest,
) -> Result<(), ApiError> {
    let id: lettuce_types::GroupId = parse_id(&request.group_id, "group_id")?;
    let expected = super::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let bytes = serde_json::to_vec(&request).map_err(|_| api_error(ApiErrorCode::Internal, "request encoding failed"))?;
    delete_and_settle(context, lettuce_database::LibraryKind::Group, id.to_string(), expected, request.client_operation_id, bytes).await
}

pub async fn character_delete(
    context: &ApiContext,
    request: dto::CharacterDeleteRequest,
) -> Result<(), ApiError> {
    let id: lettuce_types::CharacterId = parse_id(&request.character_id, "character_id")?;
    let expected = super::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let bytes = serde_json::to_vec(&request).map_err(|_| api_error(ApiErrorCode::Internal, "request encoding failed"))?;
    delete_and_settle(context, lettuce_database::LibraryKind::Character, id.to_string(), expected, request.client_operation_id, bytes).await
}

async fn delete_and_settle(
    context: &ApiContext,
    kind: lettuce_database::LibraryKind,
    id: String,
    expected: lettuce_types::Revision,
    key: String,
    bytes: Vec<u8>,
) -> Result<(), ApiError> {
    let digest = match kind {
        lettuce_database::LibraryKind::Character => blake3::hash(&bytes).to_hex().to_string(),
        lettuce_database::LibraryKind::Group => super::messages::operation(key.clone(), &[b"group_delete", &bytes])?.request_digest.to_string(),
        lettuce_database::LibraryKind::Persona => unreachable!(),
    };
    let mut changes = context.committed_changes();
    let deadline = tokio::time::Instant::now() + DELETE_SETTLE_LIMIT;
    loop {
        if context.shutdown_token().is_cancelled() {
            return Err(api_error(
                ApiErrorCode::Cancelled,
                "shutdown interrupted deletion",
            ));
        }
        changes.borrow_and_update();
        let key = key.clone();
        let id = id.clone();
        let digest = digest.clone();
        let deleted = context
            .blocking(move |context| {
                let database = context.backend().database();
                let hook = Arc::new(|_: &ApiContext| {}) as BeforeSettle;
                match kind {
                    lettuce_database::LibraryKind::Group => {
                        let group = parse_id(&id, "group_id")?;
                        if let Some(conversations) = database.group_delete_conversations(group, expected, &key, &digest).map_err(failure)? {
                            for conversation in conversations {
                                cancel_work(context, conversation, &hook)?;
                            }
                        }
                    }
                    lettuce_database::LibraryKind::Character => {
                        let character = parse_id(&id, "character_id")?;
                        if let Some(work) = database.character_delete_conversations(character, expected, &key, &digest).map_err(failure)? {
                            for conversation in work.direct_conversations {
                                cancel_work(context, conversation, &hook)?;
                            }
                            for conversation in work.group_conversations {
                                cancel_character_group_turn(context, conversation, character, &hook)?;
                            }
                        }
                    }
                    lettuce_database::LibraryKind::Persona => unreachable!(),
                }
                match database.purge_library_operation(kind, &id,
                    expected,
                    &key,
                    digest.as_str(),
                    context.now(),
                ) {
                    Ok(()) => Ok(true),
                    Err(PurgeError::Busy) => Ok(false),
                    Err(error) => Err(failure(error)),
                }
            })
            .await?;
        if deleted {
            context.wake_workers();
            context.jobs().wake();
            return context
                .blocking(|context| {
                    if let (Some(store), Some(files)) = (context.media(), context.database_files())
                    {
                        crate::collect_media_garbage(
                            context.backend().database(),
                            &crate::MediaGarbageScope {
                                store,
                                location: &files.location,
                                open_database: &files.active,
                            },
                            context.now(),
                        )
                        .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
                    }
                    Ok(())
                })
                .await;
        }
        tokio::select! {
            () = context.shutdown_token().cancelled() => { return Err(api_error(ApiErrorCode::Cancelled, "shutdown interrupted deletion")); }
            changed = changes.changed() => { changed.map_err(|_| api_error(ApiErrorCode::Internal, "change signal closed"))?; }
            () = tokio::time::sleep_until(deadline) => { return Err(api_error(ApiErrorCode::Busy, "library work did not stop")); }
        }
    }
}

pub async fn persona_delete(
    context: &ApiContext,
    request: dto::PersonaDeleteRequest,
) -> Result<(), ApiError> {
    let id: lettuce_types::PersonaId = parse_id(&request.persona_id, "persona_id")?;
    let expected = super::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let bytes = serde_json::to_vec(&request)
        .map_err(|_| api_error(ApiErrorCode::Internal, "request encoding failed"))?;
    let digest = super::messages::operation(
        request.client_operation_id.clone(),
        &[b"persona_delete", &bytes],
    )?
    .request_digest;
    if context.shutdown_token().is_cancelled() {
        return Err(api_error(
            ApiErrorCode::Cancelled,
            "shutdown interrupted deletion",
        ));
    }
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .purge_library_operation(lettuce_database::LibraryKind::Persona, &id.to_string(),
                    expected,
                    &request.client_operation_id,
                    digest.as_str(),
                    context.now(),
                )
                .map_err(failure)?;
            if let (Some(store), Some(files)) = (context.media(), context.database_files()) {
                crate::collect_media_garbage(
                    context.backend().database(),
                    &crate::MediaGarbageScope {
                        store,
                        location: &files.location,
                        open_database: &files.active,
                    },
                    context.now(),
                )
                .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
            }
            Ok(())
        })
        .await
}

pub(super) fn cancel_character_group_turn(
    context: &ApiContext,
    conversation: lettuce_types::ConversationId,
    character: lettuce_types::CharacterId,
    hook: &BeforeSettle,
) -> Result<(), ApiError> {
    use super::error::IntoApiError;
    use lettuce_conversations::{
        ConversationOverviewReader, ConversationReader, ConversationRepository,
        ConversationRepositoryError, GenerationTurnStatus, ParticipantSource,
    };
    let database = context.backend().database();
    let Some(turn_id) = ConversationOverviewReader::live_turn(database, conversation)
        .map_err(IntoApiError::into_api_error)?
    else {
        return Ok(());
    };
    let turn =
        ConversationReader::get_turn(database, turn_id).map_err(IntoApiError::into_api_error)?;
    let aggregate =
        ConversationReader::get(database, conversation).map_err(IntoApiError::into_api_error)?;
    if let Some(speaker) = &turn.selected_speaker {
        let participant = aggregate
            .conversation
            .participants
            .iter()
            .find(|participant| participant.id == speaker.participant_id)
            .ok_or_else(|| {
                api_error(
                    ApiErrorCode::Internal,
                    "selected speaker has no participant",
                )
            })?;
        if participant.source != ParticipantSource::Character(character) {
            return Ok(());
        }
    }
    if turn.status != GenerationTurnStatus::CancellationRequested {
        if !turn
            .status
            .can_transition_to(GenerationTurnStatus::CancellationRequested)
        {
            return Ok(());
        }
        let Some(attempt) = turn.attempts.iter().max_by_key(|attempt| attempt.ordinal) else {
            return Err(api_error(
                ApiErrorCode::Internal,
                "active turn has no attempt",
            ));
        };
        let operation = super::messages::operation(
            format!(
                "character-delete-cancel-{}-{}",
                turn.id,
                turn.revision.get()
            ),
            &[
                character.to_string().as_bytes(),
                turn.id.to_string().as_bytes(),
            ],
        )?;
        let command = lettuce_conversations::CancelGeneration {
            conversation_id: conversation,
            turn_id,
            attempt_id: attempt.id,
            expected_revision: aggregate.conversation.revision,
            expected_turn_revision: turn.revision,
            operation,
        };
        hook(context);
        match ConversationRepository::request_cancellation(database, &command, context.now()) {
            Ok(_) => {}
            Err(
                ConversationRepositoryError::Conflict
                | ConversationRepositoryError::StaleRevision { .. },
            ) => return Ok(()),
            Err(error) => return Err(error.into_api_error()),
        }
    }
    super::conversation_delete::cancel_turn_work(context, conversation, hook)
}
