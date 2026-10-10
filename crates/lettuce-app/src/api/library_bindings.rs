use lettuce_context::{
    BindingInsertionTarget, BindingRepositoryError, CharacterLorebookBindingRepository,
    GroupLorebookBindingRepository, LorebookBindingCreate, PersonaLorebookBindingRepository,
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_types::{CharacterId, GroupId, LorebookId, PersonaId};

use super::ApiContext;
use super::error::{api_error, invalid_field, parse_id};
use super::lorebooks::{Failure, revision};

fn binding_error(error: BindingRepositoryError) -> ApiError {
    match error {
        BindingRepositoryError::Invalid(_) => invalid_field("lorebook_binding", error.to_string()),
        BindingRepositoryError::Conflict => api_error(ApiErrorCode::Conflict, error.to_string()),
        BindingRepositoryError::NotFound => api_error(ApiErrorCode::NotFound, error.to_string()),
        BindingRepositoryError::Failure(_) => api_error(ApiErrorCode::Internal, error.to_string()),
    }
}

fn bindings(values: Vec<lettuce_context::LorebookBinding>) -> Vec<dto::LibraryLorebookBinding> {
    values
        .into_iter()
        .map(|value| dto::LibraryLorebookBinding {
            lorebook_id: value.lorebook_id.to_string(),
            enabled: value.enabled,
            ordinal: value.ordinal,
            revision: value.revision.get(),
            created_at: value.created_at.get(),
            updated_at: value.updated_at.get(),
        })
        .collect()
}

fn result(value: lettuce_context::BindingMutationResult) -> dto::LibraryLorebookMutationResult {
    dto::LibraryLorebookMutationResult {
        bindings: bindings(value.bindings),
        owner_revision: value.owner_revision.get(),
    }
}

pub async fn character_lorebooks_list(
    context: &ApiContext,
    request: dto::CharacterLorebooksListRequest,
) -> Result<Vec<dto::LibraryLorebookBinding>, ApiError> {
    let id: CharacterId = parse_id(&request.character_id, "character_id")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .list_character_bindings(id)
                .map(bindings)
                .map_err(binding_error)
        })
        .await
}

pub async fn character_lorebooks_bind(
    context: &ApiContext,
    request: dto::CharacterLorebookBindRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: CharacterId = parse_id(&request.character_id, "character_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let target = request
        .target_ordinal
        .map(|target| {
            usize::try_from(target)
                .map(BindingInsertionTarget::At)
                .map_err(|_| invalid_field("target_ordinal", "invalid ordinal"))
        })
        .transpose()?
        .unwrap_or(BindingInsertionTarget::Append);
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "character_lorebooks_bind",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .bind_character_lorebook(
                                id,
                                expected,
                                LorebookBindingCreate {
                                    lorebook_id: book,
                                    target,
                                },
                                context.now(),
                            )
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn character_lorebooks_unbind(
    context: &ApiContext,
    request: dto::CharacterLorebookUnbindRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: CharacterId = parse_id(&request.character_id, "character_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "character_lorebooks_unbind",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .unbind_character_lorebook(id, expected, book, context.now())
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn character_lorebooks_enable(
    context: &ApiContext,
    request: dto::CharacterLorebookEnableRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: CharacterId = parse_id(&request.character_id, "character_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "character_lorebooks_enable",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .set_character_lorebook_enabled(
                                id,
                                expected,
                                book,
                                request.enabled,
                                context.now(),
                            )
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn character_lorebooks_reorder(
    context: &ApiContext,
    request: dto::CharacterLorebookReorderRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: CharacterId = parse_id(&request.character_id, "character_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let target = usize::try_from(request.target_ordinal)
        .map_err(|_| invalid_field("target_ordinal", "invalid ordinal"))?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "character_lorebooks_reorder",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .reorder_character_lorebook(id, expected, book, target, context.now())
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn persona_lorebooks_list(
    context: &ApiContext,
    request: dto::PersonaLorebooksListRequest,
) -> Result<Vec<dto::LibraryLorebookBinding>, ApiError> {
    let id: PersonaId = parse_id(&request.persona_id, "persona_id")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .list_persona_bindings(id)
                .map(bindings)
                .map_err(binding_error)
        })
        .await
}

pub async fn persona_lorebooks_bind(
    context: &ApiContext,
    request: dto::PersonaLorebookBindRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: PersonaId = parse_id(&request.persona_id, "persona_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let target = request
        .target_ordinal
        .map(|target| {
            usize::try_from(target)
                .map(BindingInsertionTarget::At)
                .map_err(|_| invalid_field("target_ordinal", "invalid ordinal"))
        })
        .transpose()?
        .unwrap_or(BindingInsertionTarget::Append);
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "persona_lorebooks_bind",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .bind_persona_lorebook(
                                id,
                                expected,
                                LorebookBindingCreate {
                                    lorebook_id: book,
                                    target,
                                },
                                context.now(),
                            )
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn persona_lorebooks_unbind(
    context: &ApiContext,
    request: dto::PersonaLorebookUnbindRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: PersonaId = parse_id(&request.persona_id, "persona_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "persona_lorebooks_unbind",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .unbind_persona_lorebook(id, expected, book, context.now())
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn persona_lorebooks_enable(
    context: &ApiContext,
    request: dto::PersonaLorebookEnableRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: PersonaId = parse_id(&request.persona_id, "persona_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "persona_lorebooks_enable",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .set_persona_lorebook_enabled(
                                id,
                                expected,
                                book,
                                request.enabled,
                                context.now(),
                            )
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn persona_lorebooks_reorder(
    context: &ApiContext,
    request: dto::PersonaLorebookReorderRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: PersonaId = parse_id(&request.persona_id, "persona_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let target = usize::try_from(request.target_ordinal)
        .map_err(|_| invalid_field("target_ordinal", "invalid ordinal"))?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "persona_lorebooks_reorder",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .reorder_persona_lorebook(id, expected, book, target, context.now())
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn group_lorebooks_list(
    context: &ApiContext,
    request: dto::GroupLorebooksListRequest,
) -> Result<Vec<dto::LibraryLorebookBinding>, ApiError> {
    let id: GroupId = parse_id(&request.group_id, "group_id")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .list_group_bindings(id)
                .map(bindings)
                .map_err(binding_error)
        })
        .await
}

pub async fn group_lorebooks_bind(
    context: &ApiContext,
    request: dto::GroupLorebookBindRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: GroupId = parse_id(&request.group_id, "group_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let target = request
        .target_ordinal
        .map(|target| {
            usize::try_from(target)
                .map(BindingInsertionTarget::At)
                .map_err(|_| invalid_field("target_ordinal", "invalid ordinal"))
        })
        .transpose()?
        .unwrap_or(BindingInsertionTarget::Append);
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "group_lorebooks_bind",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .bind_group_lorebook(
                                id,
                                expected,
                                LorebookBindingCreate {
                                    lorebook_id: book,
                                    target,
                                },
                                context.now(),
                            )
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn group_lorebooks_unbind(
    context: &ApiContext,
    request: dto::GroupLorebookUnbindRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: GroupId = parse_id(&request.group_id, "group_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "group_lorebooks_unbind",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .unbind_group_lorebook(id, expected, book, context.now())
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn group_lorebooks_enable(
    context: &ApiContext,
    request: dto::GroupLorebookEnableRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: GroupId = parse_id(&request.group_id, "group_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "group_lorebooks_enable",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .set_group_lorebook_enabled(
                                id,
                                expected,
                                book,
                                request.enabled,
                                context.now(),
                            )
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn group_lorebooks_reorder(
    context: &ApiContext,
    request: dto::GroupLorebookReorderRequest,
) -> Result<dto::LibraryLorebookMutationResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id: GroupId = parse_id(&request.group_id, "group_id")?;
    let book: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let target = usize::try_from(request.target_ordinal)
        .map_err(|_| invalid_field("target_ordinal", "invalid ordinal"))?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "group_lorebooks_reorder",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .reorder_group_lorebook(id, expected, book, target, context.now())
                            .map(result)
                            .map_err(|error| Failure(binding_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}
