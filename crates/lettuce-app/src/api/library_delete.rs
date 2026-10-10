use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::PurgeError;
use lettuce_types::GroupId;

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
    let id: GroupId = parse_id(&request.group_id, "group_id")?;
    let expected = super::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let bytes = serde_json::to_vec(&request)
        .map_err(|_| api_error(ApiErrorCode::Internal, "request encoding failed"))?;
    let digest = super::messages::operation(
        request.client_operation_id.clone(),
        &[b"group_delete", &bytes],
    )?
    .request_digest;
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
        let key = request.client_operation_id.clone();
        let digest = digest.clone();
        let deleted = context
            .blocking(move |context| {
                let database = context.backend().database();
                if let Some(conversations) = database
                    .group_delete_conversations(id, expected, &key, digest.as_str())
                    .map_err(failure)?
                {
                    let hook = Arc::new(|_: &ApiContext| {}) as BeforeSettle;
                    for conversation in conversations {
                        cancel_work(context, conversation, &hook)?;
                    }
                }
                match database.purge_group_operation(
                    id,
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
            () = tokio::time::sleep_until(deadline) => { return Err(api_error(ApiErrorCode::Busy, "group work did not stop")); }
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
                .purge_persona_operation(
                    id,
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
