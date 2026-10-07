use lettuce_characters::{CharacterRepository, GroupRepository};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode, RequiredModel};
use lettuce_conversations::{
    BranchStatus, ConversationKind, ConversationLifecycle, ConversationReader,
};
use lettuce_database::ApiOperationError;
use lettuce_embeddings::{EmbeddingRequest, MemoryEmbeddingProjection};
use lettuce_jobs::handle::CancellationToken;
use lettuce_memory::{
    MemoryContextRevision, MemoryFieldChange, MemoryItem, MemoryManualEdit, MemoryManualMutation,
    MemoryOrigin, MemoryRepository, MemoryRepositoryError, MemoryShortId, MemorySummary,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{ConversationId, MemoryId, OperationId, TimestampMillis};

use super::error::{IntoApiError, api_error, invalid_field, model_error, parse_id};
use super::{ApiContext, messages};

pub(super) struct EditFailure(pub(super) ApiError);

impl From<ApiOperationError> for EditFailure {
    fn from(error: ApiOperationError) -> Self {
        Self(api_error(
            if error == ApiOperationError::Conflict {
                ApiErrorCode::Conflict
            } else {
                ApiErrorCode::Internal
            },
            error.to_string(),
        ))
    }
}

impl From<MemoryRepositoryError> for EditFailure {
    fn from(error: MemoryRepositoryError) -> Self {
        Self(memory_error(error))
    }
}

pub(crate) fn memory_error(error: MemoryRepositoryError) -> ApiError {
    api_error(
        match error {
            MemoryRepositoryError::NotFound => ApiErrorCode::NotFound,
            MemoryRepositoryError::Conflict | MemoryRepositoryError::AlreadyExists => {
                ApiErrorCode::Conflict
            }
            MemoryRepositoryError::Invalid(_) => ApiErrorCode::InvalidInput,
            MemoryRepositoryError::Failure(_) => ApiErrorCode::Internal,
        },
        error.to_string(),
    )
}

fn category(value: dto::MemoryCategory) -> lettuce_memory::MemoryCategory {
    use dto::MemoryCategory as D;
    use lettuce_memory::MemoryCategory as M;
    match value {
        D::CharacterTrait => M::CharacterTrait,
        D::Relationship => M::Relationship,
        D::PlotEvent => M::PlotEvent,
        D::WorldDetail => M::WorldDetail,
        D::Preference => M::Preference,
        D::Other => M::Other,
        D::Milestone => M::Milestone,
        D::Boundary => M::Boundary,
        D::Profile => M::Profile,
        D::Routine => M::Routine,
        D::Episodic => M::Episodic,
        D::EmotionalSnapshot => M::EmotionalSnapshot,
    }
}

fn embedding_error(context: &ApiContext, error: crate::EmbeddingGenerationError) -> ApiError {
    match error {
        crate::EmbeddingGenerationError::Cancelled => {
            api_error(ApiErrorCode::Cancelled, "memory embedding was cancelled")
        }
        crate::EmbeddingGenerationError::Unavailable => model_error(
            if super::models::installed_models(context).contains(&RequiredModel::Embedding) {
                ApiErrorCode::ModelUnavailable
            } else {
                ApiErrorCode::ModelRequired
            },
            RequiredModel::Embedding,
        ),
    }
}

pub(super) fn live_conversation(
    database: &lettuce_database::Database,
    conversation_id: ConversationId,
) -> Result<lettuce_conversations::Conversation, ApiError> {
    let aggregate =
        ConversationReader::get(database, conversation_id).map_err(IntoApiError::into_api_error)?;
    let conversation = aggregate.conversation;
    if conversation.lifecycle == ConversationLifecycle::Tombstoned
        || !aggregate.branches.iter().any(|branch| {
            branch.id == conversation.active_branch_id && branch.status == BranchStatus::Active
        })
    {
        return Err(api_error(
            ApiErrorCode::NotFound,
            "the active conversation branch is unavailable",
        ));
    }
    Ok(conversation)
}

enum Edit {
    Add(dto::MemoryAddRequest),
    Update(dto::MemoryUpdateRequest),
    Delete(dto::MemoryDeleteRequest),
    Pin(dto::MemoryPinRequest),
    Temperature(dto::MemoryTemperatureRequest),
    Summary(dto::MemorySummaryUpdateRequest),
}

async fn apply<R: serde::Serialize>(
    context: &ApiContext,
    command: &'static str,
    conversation_id: &str,
    revision: u64,
    key: &str,
    request: &R,
    edit: Edit,
) -> Result<dto::MemoryEditResult, ApiError> {
    let conversation_id: ConversationId = parse_id(conversation_id, "conversation_id")?;
    let expected_revision = messages::expected_revision(revision)?;
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(request).map_err(|_| {
        api_error(
            ApiErrorCode::Internal,
            "memory request could not be encoded",
        )
    })?);
    let operation = messages::operation(key.to_owned(), &[command.as_bytes(), &bytes])?;
    let key = key.to_owned();
    context
        .blocking(move |context| {
            let database = context.backend().database();
            if let Some(receipt) = database
                .lookup_api_operation(command, &key)
                .map_err(|error| EditFailure::from(error).0)?
            {
                if receipt.request_digest != operation.request_digest.as_str() {
                    return Err(api_error(
                        ApiErrorCode::Conflict,
                        "the operation key was used for another request",
                    ));
                }
                return serde_json::from_value(receipt.result)
                    .map_err(|_| api_error(ApiErrorCode::Internal, "memory receipt is invalid"));
            }
            let aggregate = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            let conversation = &aggregate.conversation;
            if conversation.lifecycle == ConversationLifecycle::Tombstoned
                || !aggregate.branches.iter().any(|branch| {
                    branch.id == conversation.active_branch_id
                        && branch.status == BranchStatus::Active
                })
            {
                return Err(api_error(
                    ApiErrorCode::NotFound,
                    "the active conversation branch is unavailable",
                ));
            }
            let memory = database
                .get_for_branch(conversation_id, conversation.active_branch_id)
                .map_err(memory_error)?
                .ok_or_else(|| {
                    api_error(
                        ApiErrorCode::NotFound,
                        "the conversation memory space is unavailable",
                    )
                })?;
            if memory.revision != expected_revision {
                return Err(api_error(ApiErrorCode::Conflict, "memory revision changed"));
            }
            let settings = GlobalSettingsStore::load(database)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            let mut context_revisions = vec![MemoryContextRevision::Settings {
                revision: settings.revision,
            }];
            match &conversation.kind {
                ConversationKind::Direct(details) => {
                    if let Some(character) =
                        CharacterRepository::get(database, details.character.source_id)
                            .map_err(IntoApiError::into_api_error)?
                    {
                        context_revisions.push(MemoryContextRevision::Character {
                            id: character.character.id,
                            revision: character.character.revision,
                        });
                    }
                }
                ConversationKind::Group(details) => {
                    if let Some(group) = GroupRepository::get(database, details.group.source_id)
                        .map_err(IntoApiError::into_api_error)?
                    {
                        context_revisions.push(MemoryContextRevision::Group {
                            id: group.group.id,
                            revision: group.group.revision,
                        });
                    }
                }
            }
            let dynamic = crate::companion::companion_memory_host::dynamic_memory_on(
                database,
                conversation,
                &settings.settings,
            )
            .map_err(IntoApiError::into_api_error)?;
            let companion =
                crate::companion::companion_clock::companion_clock_context(database, conversation)
                    .map_err(|_| {
                        api_error(
                            ApiErrorCode::Internal,
                            "the conversation mode could not be resolved",
                        )
                    })?
                    .companion;
            let validate_category =
                |value: Option<lettuce_memory::MemoryCategory>| -> Result<(), ApiError> {
                    if !companion
                        && value.is_some_and(|value| {
                            !matches!(
                                value,
                                lettuce_memory::MemoryCategory::CharacterTrait
                                    | lettuce_memory::MemoryCategory::Relationship
                                    | lettuce_memory::MemoryCategory::PlotEvent
                                    | lettuce_memory::MemoryCategory::WorldDetail
                                    | lettuce_memory::MemoryCategory::Preference
                                    | lettuce_memory::MemoryCategory::Other
                            )
                        })
                    {
                        return Err(invalid_field(
                            "category",
                            "the category requires companion mode",
                        ));
                    }
                    Ok(())
                };
            let now = context.now();
            let engine = context.embedding();
            let count = |text: &str, required: bool| -> Result<Option<u32>, ApiError> {
                match engine.count_tokens(text) {
                    Ok(value) => Ok(Some(value)),
                    Err(crate::EmbeddingGenerationError::Unavailable) if !required => Ok(None),
                    Err(error) => Err(embedding_error(context, error)),
                }
            };
            let find = |value: &str| -> Result<MemoryId, ApiError> {
                let id = parse_id(value, "memory_id")?;
                memory
                    .items
                    .iter()
                    .any(|item| item.id == id)
                    .then_some(id)
                    .ok_or_else(|| {
                        api_error(
                            ApiErrorCode::NotFound,
                            "the memory does not belong to the conversation's active space",
                        )
                    })
            };
            let mutation = match edit {
                Edit::Add(request) => {
                    let category = request.category.map(category);
                    validate_category(category)?;
                    let id = MemoryId::new();
                    let mut item = MemoryItem::written(
                        id,
                        MemoryShortId::allocate(id, |short| {
                            memory.items.iter().any(|item| item.short_id == short)
                        }),
                        request.text.trim().to_owned(),
                        now,
                    );
                    item.category = category;
                    item.observed_at = request.observed_at.map(TimestampMillis::new);
                    item.observed_time_precision = item.observed_at.map(|_| "user".into());
                    item.token_count = count(&item.text, dynamic)?;
                    MemoryManualMutation::Add { item }
                }
                Edit::Update(request) => {
                    let memory_id = find(&request.memory_id)?;
                    let category = match request.category {
                        dto::MemoryCategoryChange::Keep => MemoryFieldChange::Keep,
                        dto::MemoryCategoryChange::Set(value) => {
                            let value = value.map(category);
                            validate_category(value)?;
                            MemoryFieldChange::Set(value)
                        }
                    };
                    let observed_at = match request.observed_at {
                        dto::MemoryObservedAtChange::Keep => MemoryFieldChange::Keep,
                        dto::MemoryObservedAtChange::Set(value) => {
                            MemoryFieldChange::Set(value.map(TimestampMillis::new))
                        }
                    };
                    let text = request.text.map(|text| text.trim().to_owned());
                    if dynamic && text.is_none() {
                        let stored = memory
                            .items
                            .iter()
                            .find(|item| item.id == memory_id)
                            .ok_or_else(|| {
                                api_error(ApiErrorCode::NotFound, "memory item is unavailable")
                            })?;
                        count(&stored.text, true)?;
                    }
                    let token_count = text
                        .as_deref()
                        .map(|text| count(text, dynamic))
                        .transpose()?
                        .flatten();
                    MemoryManualMutation::Update {
                        memory_id,
                        text,
                        category,
                        observed_at,
                        token_count,
                    }
                }
                Edit::Delete(request) => MemoryManualMutation::Delete {
                    memory_id: find(&request.memory_id)?,
                },
                Edit::Pin(request) => MemoryManualMutation::Pin {
                    memory_id: find(&request.memory_id)?,
                    pinned: request.pinned,
                },
                Edit::Temperature(request) => MemoryManualMutation::Temperature {
                    memory_id: find(&request.memory_id)?,
                    cold: request.temperature == dto::MemoryTemperature::Cold,
                },
                Edit::Summary(request) => MemoryManualMutation::Summary {
                    summary: match request.summary {
                        dto::MemorySummaryEdit::Clear => None,
                        dto::MemorySummaryEdit::Set { text } => {
                            let text = text.trim().to_owned();
                            let token_count = count(&text, false)?;
                            Some(MemorySummary {
                                origin: MemoryOrigin::User,
                                space_id: memory.id,
                                branch_id: conversation.active_branch_id,
                                text,
                                token_count,
                                window_start: 0,
                                window_end: 0,
                                source_message_ids: vec![],
                                updated_at: now,
                            })
                        }
                    },
                },
            };
            lettuce_memory::reduce_manual_memory(&memory, &mutation, now).map_err(memory_error)?;
            let text_item = match &mutation {
                MemoryManualMutation::Add { item } => Some((item.id, item.text.as_str())),
                MemoryManualMutation::Update {
                    memory_id,
                    text: Some(text),
                    ..
                } => Some((*memory_id, text.as_str())),
                _ => None,
            };
            let projection = if dynamic {
                text_item
                    .map(|(memory_id, text)| {
                        let vector = engine
                            .embed_memory(
                                &EmbeddingRequest {
                                    text: text.to_owned(),
                                    dimensions: engine.dimensions(),
                                },
                                &CancellationToken::new(),
                            )
                            .map_err(|error| embedding_error(context, error))?;
                        let projection = MemoryEmbeddingProjection {
                            space_id: memory.id,
                            memory_id,
                            source_text: text.to_owned(),
                            vector,
                            dimensions: engine.dimensions(),
                            updated_at: now,
                        };
                        projection.validate().map_err(|_| {
                            api_error(
                                ApiErrorCode::ModelUnavailable,
                                "memory embedding returned an invalid vector",
                            )
                        })?;
                        Ok::<_, ApiError>(projection)
                    })
                    .transpose()?
            } else {
                None
            };
            let edit = MemoryManualEdit {
                id: OperationId::new(),
                conversation_id,
                branch_id: conversation.active_branch_id,
                conversation_revision: conversation.revision,
                expected_revision,
                space_id: memory.id,
                context_revisions,
                mutation,
                at: now,
            };
            database
                .commit_api_operation(
                    command,
                    &key,
                    operation.request_digest.as_str(),
                    now,
                    |scope| {
                        let history = scope
                            .apply_memory_manual_edit(&edit, projection.as_ref())
                            .map_err(EditFailure::from)?;
                        Ok(dto::MemoryEditResult {
                            revision: history.resulting_revision.get(),
                            memory_id: history
                                .after_item
                                .as_ref()
                                .or(history.before_item.as_ref())
                                .map(|item| item.id.to_string()),
                        })
                    },
                )
                .map_err(|error: EditFailure| error.0)
        })
        .await
}

pub async fn memory_add(
    context: &ApiContext,
    request: dto::MemoryAddRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    apply(
        context,
        "memory_add",
        &request.conversation_id,
        request.expected_revision,
        &request.client_operation_id,
        &request,
        Edit::Add(request.clone()),
    )
    .await
}
pub async fn memory_update(
    context: &ApiContext,
    request: dto::MemoryUpdateRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    apply(
        context,
        "memory_update",
        &request.conversation_id,
        request.expected_revision,
        &request.client_operation_id,
        &request,
        Edit::Update(request.clone()),
    )
    .await
}
pub async fn memory_delete(
    context: &ApiContext,
    request: dto::MemoryDeleteRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    apply(
        context,
        "memory_delete",
        &request.conversation_id,
        request.expected_revision,
        &request.client_operation_id,
        &request,
        Edit::Delete(request.clone()),
    )
    .await
}
pub async fn memory_pin(
    context: &ApiContext,
    request: dto::MemoryPinRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    apply(
        context,
        "memory_pin",
        &request.conversation_id,
        request.expected_revision,
        &request.client_operation_id,
        &request,
        Edit::Pin(request.clone()),
    )
    .await
}
pub async fn memory_set_temperature(
    context: &ApiContext,
    request: dto::MemoryTemperatureRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    apply(
        context,
        "memory_set_temperature",
        &request.conversation_id,
        request.expected_revision,
        &request.client_operation_id,
        &request,
        Edit::Temperature(request.clone()),
    )
    .await
}
pub async fn memory_summary_update(
    context: &ApiContext,
    request: dto::MemorySummaryUpdateRequest,
) -> Result<dto::MemoryEditResult, ApiError> {
    apply(
        context,
        "memory_summary_update",
        &request.conversation_id,
        request.expected_revision,
        &request.client_operation_id,
        &request,
        Edit::Summary(request.clone()),
    )
    .await
}
