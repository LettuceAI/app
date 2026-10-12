use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_sync::{ConflictRepositoryError, ConversationForkRepository, SyncConflictRepository};

use crate::api::{ApiContext, error::parse_id, lorebooks::Failure};

fn conflict_error(error: ConflictRepositoryError) -> ApiError {
    let code = match error {
        ConflictRepositoryError::NotFound => ApiErrorCode::NotFound,
        ConflictRepositoryError::Conflict => ApiErrorCode::Conflict,
        ConflictRepositoryError::Unsupported => ApiErrorCode::Unsupported,
        ConflictRepositoryError::Unavailable => ApiErrorCode::Unavailable,
        ConflictRepositoryError::Corrupt | ConflictRepositoryError::Storage => {
            ApiErrorCode::Internal
        }
    };
    ApiError {
        code,
        message: error.to_string(),
        details: Some(dto::ApiErrorDetails::Sync {
            reason: match error {
                ConflictRepositoryError::Unavailable => {
                    dto::SyncFailure::RequiredReferenceUnavailable
                }
                _ => dto::SyncFailure::Storage,
            },
        }),
    }
}

fn candidate(
    context: &ApiContext,
    candidate: lettuce_sync::SyncConflictCandidate,
) -> Result<dto::SyncConflictCandidate, ApiError> {
    use lettuce_sync::SyncConflictValue as V;
    let value = match candidate.value {
        V::Character(value) => dto::SyncConflictValue::Character {
            value: Box::new(crate::api::library_characters::view(context, *value)?),
        },
        V::Persona(value) => dto::SyncConflictValue::Persona {
            value: Box::new(crate::api::library_personas::view(context, value)),
        },
        V::Default(value) => dto::SyncConflictValue::PersonaDefault {
            value: dto::PersonaDefaultView {
                persona_id: value.persona_id.map(|id| id.to_string()),
                revision: value.revision.get(),
                created_at: value.created_at.get(),
                updated_at: value.updated_at.get(),
            },
        },
        V::Lorebook(value) => dto::SyncConflictValue::Lorebook {
            value: crate::api::lorebooks::view(context, &value),
        },
        V::Prompt(value) => dto::SyncConflictValue::Prompt {
            value: crate::api::prompts::view(context, &value)?,
        },
        V::Deleted => dto::SyncConflictValue::Deleted,
    };
    Ok(dto::SyncConflictCandidate {
        change_id: candidate.change_id.map(|id| id.as_uuid().to_string()),
        device_id: candidate.device_id.map(|id| id.as_uuid().to_string()),
        timestamp: candidate.timestamp.map(|at| at.get()),
        value,
    })
}

pub async fn sync_conflicts_list(context: &ApiContext) -> Result<dto::SyncConflictsList, ApiError> {
    context
        .blocking(|context| {
            let database = context.backend().database();
            let conflicts = database
                .unresolved_sync_conflicts()
                .map_err(conflict_error)?
                .into_iter()
                .map(|conflict| {
                    Ok(dto::SyncConflictView {
                        conflict_id: conflict.id.to_string(),
                        entity_kind: conflict.entity_kind,
                        entity_id: conflict.entity_id,
                        detected_at: conflict.detected_at.get(),
                        current: candidate(context, conflict.current)?,
                        other: candidate(context, conflict.other)?,
                    })
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            let legacy_conflicts = database
                .unresolved_legacy_conflicts()
                .map_err(conflict_error)?
                .into_iter()
                .map(|conflict| dto::LegacyConflictView {
                    run_id: conflict.run_id.to_string(),
                    source_key: conflict.source_key,
                    entity_kind: conflict.entity_kind,
                    entity_id: conflict.entity_id,
                    detected_at: conflict.detected_at.get(),
                    fields: conflict
                        .fields
                        .into_iter()
                        .map(|field| dto::LegacyConflictField {
                            key: field.key,
                            current: match field.current {
                                lettuce_database::LegacyConflictCurrent::Value(value) => {
                                    dto::LegacyConflictCurrent::Value { value }
                                }
                                lettuce_database::LegacyConflictCurrent::NotMapped => {
                                    dto::LegacyConflictCurrent::NotMapped
                                }
                            },
                            local: field.local,
                            incoming: field.incoming,
                        })
                        .collect(),
                })
                .collect();
            let forks = database
                .unresolved_conversation_forks()
                .map_err(conflict_error)?
                .into_iter()
                .map(|fork| dto::SyncForkView {
                    conversation_id: fork.conversation_id.to_string(),
                    branch_id: fork.branch_id.to_string(),
                    holds_local: fork.holds_local,
                    detected_at: fork.detected_at.get(),
                })
                .collect();
            Ok(dto::SyncConflictsList {
                conflicts,
                legacy_conflicts,
                forks,
            })
        })
        .await
}

pub async fn sync_conflict_resolve(
    context: &ApiContext,
    request: dto::SyncConflictResolveRequest,
) -> Result<dto::SyncConflictResolution, ApiError> {
    let conflict_id = parse_id(&request.conflict_id, "conflict_id")?;
    let expected = request
        .expected_current_change
        .as_deref()
        .map(|id| {
            uuid::Uuid::parse_str(id)
                .map(lettuce_sync::SyncChangeId::from_uuid)
                .map_err(|_| {
                    crate::api::error::invalid_field("expected_current_change", "Invalid change id")
                })
        })
        .transpose()?;
    let choice = match request.choice {
        dto::SyncConflictChoice::Current => lettuce_sync::ConflictChoice::Current,
        dto::SyncConflictChoice::Other => lettuce_sync::ConflictChoice::Other,
    };
    let digest = crate::api::jobs::local::digest(&request)?;
    crate::api::jobs::local::operation_key("sync_conflict_resolve", &request.client_operation_id)?;
    let key = request.client_operation_id;
    let cancellation = context.shutdown_token();
    let _lease = context
        .maintenance()
        .work(cancellation)
        .await
        .ok_or_else(|| {
            crate::api::error::api_error(
                ApiErrorCode::Cancelled,
                "Shutdown interrupted conflict resolution",
            )
        })?;
    context
        .blocking(move |context| {
            if context.shutdown_token().is_cancelled() {
                return Err(crate::api::error::api_error(
                    ApiErrorCode::Cancelled,
                    "Shutdown interrupted conflict resolution",
                ));
            }
            context
                .backend()
                .database()
                .commit_api_operation(
                    "sync_conflict_resolve",
                    &key,
                    &digest,
                    context.now(),
                    |tx| {
                        let result = tx
                            .resolve_sync_conflict(
                                conflict_id,
                                expected,
                                choice,
                                lettuce_types::OperationId::new(),
                                context.now(),
                            )
                            .map_err(|error| Failure(conflict_error(error)))?;
                        Ok(dto::SyncConflictResolution {
                            change_id: result.id().as_uuid().to_string(),
                        })
                    },
                )
                .map_err(|error: Failure| error.0)
        })
        .await
}

pub async fn sync_fork_resolve(
    context: &ApiContext,
    request: dto::SyncForkResolveRequest,
) -> Result<(), ApiError> {
    let conversation_id = parse_id(&request.conversation_id, "conversation_id")?;
    let branch_id = parse_id(&request.branch_id, "branch_id")?;
    let digest = crate::api::jobs::local::digest(&request)?;
    crate::api::jobs::local::operation_key("sync_fork_resolve", &request.client_operation_id)?;
    let key = request.client_operation_id;
    let _lease = context
        .maintenance()
        .work(context.shutdown_token())
        .await
        .ok_or_else(|| {
            crate::api::error::api_error(
                ApiErrorCode::Cancelled,
                "Shutdown interrupted fork resolution",
            )
        })?;
    context
        .blocking(move |context| {
            if context.shutdown_token().is_cancelled() {
                return Err(crate::api::error::api_error(
                    ApiErrorCode::Cancelled,
                    "Shutdown interrupted fork resolution",
                ));
            }
            context
                .backend()
                .database()
                .commit_api_operation("sync_fork_resolve", &key, &digest, context.now(), |tx| {
                    tx.resolve_conversation_fork(conversation_id, branch_id, context.now())
                        .map_err(|error| Failure(conflict_error(error)))
                })
                .map_err(|error: Failure| error.0)
        })
        .await
}

pub async fn legacy_conflict_dismiss(
    context: &ApiContext,
    request: dto::LegacyConflictDismissRequest,
) -> Result<(), ApiError> {
    let run_id = parse_id(&request.run_id, "run_id")?;
    if request.source_key.trim().is_empty() {
        return Err(crate::api::error::invalid_field(
            "source_key",
            "A source key is required",
        ));
    }
    let digest = crate::api::jobs::local::digest(&request)?;
    crate::api::jobs::local::operation_key(
        "legacy_conflict_dismiss",
        &request.client_operation_id,
    )?;
    let key = request.client_operation_id;
    let _lease = context
        .maintenance()
        .work(context.shutdown_token())
        .await
        .ok_or_else(|| {
            crate::api::error::api_error(
                ApiErrorCode::Cancelled,
                "Shutdown interrupted legacy dismissal",
            )
        })?;
    context
        .blocking(move |context| {
            if context.shutdown_token().is_cancelled() {
                return Err(crate::api::error::api_error(
                    ApiErrorCode::Cancelled,
                    "Shutdown interrupted legacy dismissal",
                ));
            }
            context
                .backend()
                .database()
                .commit_api_operation(
                    "legacy_conflict_dismiss",
                    &key,
                    &digest,
                    context.now(),
                    |tx| {
                        tx.dismiss_legacy_conflict(run_id, &request.source_key, context.now())
                            .map_err(|error| Failure(conflict_error(error)))
                    },
                )
                .map_err(|error: Failure| error.0)
        })
        .await
}
