//! Lorebook library commands: reads, authored writes, archive, hard delete,
//! the trigger preview and token counts.

use lettuce_context::{
    DetectionPolicy, KeywordMatchMode, LifecycleStatus, Lorebook, LorebookBehaviorVersion,
    LorebookDetails, LorebookEntry, LorebookEntryDraft, LorebookEntryInsertionTarget,
    LorebookEntryMutation, LorebookLibraryQuery, LorebookMetadataDraft, LorebookRepository,
    LorebookSourceProvenance, LorebookSourceSkipReason,
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::ApiOperationError;
use lettuce_types::{AssetId, CharacterId, LorebookEntryId, LorebookId, PageRequest, Revision};

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::mapping;
use super::messages::operation;

pub(super) struct Failure(pub(super) ApiError);

impl From<ApiOperationError> for Failure {
    fn from(error: ApiOperationError) -> Self {
        Self(api_error(
            match error {
                ApiOperationError::Conflict => ApiErrorCode::Conflict,
                ApiOperationError::InvalidData | ApiOperationError::Storage => {
                    ApiErrorCode::Internal
                }
            },
            error.to_string(),
        ))
    }
}

pub(super) fn lifecycle(filter: Option<dto::LifecycleFilter>) -> lettuce_context::LifecycleFilter {
    match filter.unwrap_or_default() {
        dto::LifecycleFilter::Active => lettuce_context::LifecycleFilter::Active,
        dto::LifecycleFilter::Archived => lettuce_context::LifecycleFilter::Archived,
        dto::LifecycleFilter::All => lettuce_context::LifecycleFilter::All,
    }
}

pub(super) fn revision(value: u64, field: &str) -> Result<Revision, ApiError> {
    if value == 0 {
        return Err(invalid_field(field, "revisions start at 1"));
    }
    Ok(Revision::new(value))
}

fn summary(context: &ApiContext, book: &Lorebook) -> dto::LorebookSummary {
    dto::LorebookSummary {
        id: book.id.to_string(),
        name: book.name.clone(),
        status: match book.status {
            LifecycleStatus::Active => dto::LorebookStatus::Active,
            LifecycleStatus::Archived => dto::LorebookStatus::Archived,
        },
        detection: match book.detection_policy {
            DetectionPolicy::RecentMessageWindow => dto::LorebookDetection::RecentMessages,
            DetectionPolicy::LatestUserMessage => dto::LorebookDetection::LatestUserMessage,
        },
        icon: book.icon_asset_id.map(|asset| context.asset_ref(asset)),
        revision: book.revision.get(),
        created_at: book.created_at.get(),
        updated_at: book.updated_at.get(),
    }
}

fn entry_view(entry: &LorebookEntry) -> dto::LorebookEntryView {
    dto::LorebookEntryView {
        id: entry.id.to_string(),
        title: entry.title.clone(),
        enabled: entry.enabled,
        always_active: entry.always_active,
        keywords: entry.keywords.clone(),
        case_sensitive: entry.case_sensitive,
        keyword_mode: match entry.match_mode {
            KeywordMatchMode::Literal => dto::LorebookKeywordMode::Literal,
            KeywordMatchMode::Regex => dto::LorebookKeywordMode::Regex,
        },
        content: entry.content.clone(),
        priority: entry.priority,
        ordinal: entry.ordinal,
        revision: entry.revision.get(),
        created_at: entry.created_at.get(),
        updated_at: entry.updated_at.get(),
    }
}

pub(super) fn view(context: &ApiContext, details: &LorebookDetails) -> dto::LorebookView {
    dto::LorebookView {
        lorebook: summary(context, &details.book),
        entries: details.entries.iter().map(entry_view).collect(),
    }
}

fn metadata(input: dto::LorebookMetadataInput) -> Result<LorebookMetadataDraft, ApiError> {
    Ok(LorebookMetadataDraft {
        name: input.name,
        detection_policy: match input.detection {
            dto::LorebookDetection::RecentMessages => DetectionPolicy::RecentMessageWindow,
            dto::LorebookDetection::LatestUserMessage => DetectionPolicy::LatestUserMessage,
        },
        icon_asset_id: input
            .icon_asset_id
            .as_deref()
            .map(|id| parse_id::<AssetId>(id, "icon_asset_id"))
            .transpose()?,
        behavior_version: LorebookBehaviorVersion::LegacyV1,
    })
}

pub(super) fn entry_draft(input: dto::LorebookEntryInput) -> LorebookEntryDraft {
    LorebookEntryDraft {
        title: input.title,
        enabled: input.enabled,
        always_active: input.always_active,
        keywords: input.keywords,
        case_sensitive: input.case_sensitive,
        match_mode: match input.keyword_mode {
            dto::LorebookKeywordMode::Literal => KeywordMatchMode::Literal,
            dto::LorebookKeywordMode::Regex => KeywordMatchMode::Regex,
        },
        content: input.content,
        priority: input.priority,
    }
}

fn index(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn mutation(input: dto::LorebookEntryMutationInput) -> Result<LorebookEntryMutation, ApiError> {
    Ok(match input {
        dto::LorebookEntryMutationInput::Add { entry, index: at } => LorebookEntryMutation::Add {
            draft: entry_draft(entry),
            target: at.map_or(LorebookEntryInsertionTarget::Append, |at| {
                LorebookEntryInsertionTarget::At(index(at))
            }),
        },
        dto::LorebookEntryMutationInput::Update { entry_id, entry } => {
            LorebookEntryMutation::Update {
                entry_id: parse_id::<LorebookEntryId>(&entry_id, "entry_id")?,
                draft: entry_draft(entry),
            }
        }
        dto::LorebookEntryMutationInput::Remove { entry_id } => LorebookEntryMutation::Remove {
            entry_id: parse_id(&entry_id, "entry_id")?,
        },
        dto::LorebookEntryMutationInput::Reorder {
            entry_id,
            index: at,
        } => LorebookEntryMutation::Reorder {
            entry_id: parse_id(&entry_id, "entry_id")?,
            target_index: index(at),
        },
    })
}

pub(super) fn deletion(deletion: lettuce_database::SourceDeletion) -> dto::SourceDeleteResult {
    dto::SourceDeleteResult {
        character_ids: deletion
            .characters
            .iter()
            .map(ToString::to_string)
            .collect(),
        persona_ids: deletion.personas.iter().map(ToString::to_string).collect(),
        group_ids: deletion.groups.iter().map(ToString::to_string).collect(),
        conversation_ids: deletion
            .conversations
            .iter()
            .map(ToString::to_string)
            .collect(),
        settings_changed: deletion.settings_changed,
    }
}

pub async fn lorebooks_list(
    context: &ApiContext,
    request: dto::LorebooksListRequest,
) -> Result<dto::LorebookPage, ApiError> {
    context
        .blocking(move |context| {
            let page = LorebookRepository::page(
                context.backend().database(),
                LorebookLibraryQuery {
                    page: PageRequest {
                        cursor: request.cursor,
                        limit: mapping::page_limit(request.limit),
                    },
                    status: lifecycle(request.lifecycle),
                    name_contains: request.query,
                },
            )
            .map_err(|error| match error {
                lettuce_context::LorebookRepositoryError::Failure(_) => {
                    invalid_field("cursor", "the cursor is not valid")
                }
                error => error.into_api_error(),
            })?;
            Ok(dto::LorebookPage {
                items: page
                    .items
                    .iter()
                    .map(|book| summary(context, book))
                    .collect(),
                next_cursor: page.next_cursor,
            })
        })
        .await
}

pub async fn lorebook_get(
    context: &ApiContext,
    request: dto::LorebookGetRequest,
) -> Result<dto::LorebookView, ApiError> {
    let id: LorebookId = parse_id(&request.lorebook_id, "lorebook_id")?;
    context
        .blocking(move |context| {
            let details = LorebookRepository::get(context.backend().database(), id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "lorebook not found"))?;
            Ok(view(context, &details))
        })
        .await
}

/// Creates a lorebook with its entries. The operation key makes a retry
/// return the first result; the same key with another request conflicts.
pub async fn lorebook_create(
    context: &ApiContext,
    request: dto::LorebookCreateRequest,
) -> Result<dto::LorebookView, ApiError> {
    let bytes = serde_json::to_vec(&request)
        .map_err(|_| api_error(ApiErrorCode::Internal, "the request could not be encoded"))?;
    let token = operation(
        request.client_operation_id.clone(),
        &[b"lorebook_create", &bytes],
    )?;
    let draft = metadata(request.metadata)?;
    let entries = request
        .entries
        .into_iter()
        .map(entry_draft)
        .collect::<Vec<_>>();
    let key = request.client_operation_id;
    context
        .blocking(move |context| {
            let now = context.now();
            context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_create",
                    &key,
                    token.request_digest.as_str(),
                    now,
                    |scope| {
                        let details = scope
                            .create_lorebook(draft, entries, now)
                            .map_err(|error| Failure(error.into_api_error()))?;
                        Ok::<_, Failure>(view(context, &details))
                    },
                )
                .map_err(|failure| failure.0)
        })
        .await
}

pub async fn lorebook_update_metadata(
    context: &ApiContext,
    request: dto::LorebookUpdateMetadataRequest,
) -> Result<dto::LorebookView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let metadata = metadata(request.metadata)?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_update_metadata",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        let details = scope
                            .update_lorebook_metadata(id, expected, metadata, context.now())
                            .map_err(|error| Failure(error.into_api_error()))?;
                        Ok::<_, Failure>(view(context, &details))
                    },
                )
                .map_err(|error| error.0)
        })
        .await
}

pub async fn lorebook_entries_mutate(
    context: &ApiContext,
    request: dto::LorebookEntriesMutateRequest,
) -> Result<dto::LorebookView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let mutations = request
        .mutations
        .into_iter()
        .map(mutation)
        .collect::<Result<Vec<_>, _>>()?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_entries_mutate",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        let details = scope
                            .mutate_lorebook_entries(id, expected, mutations, context.now())
                            .map_err(|error| Failure(error.into_api_error()))?;
                        Ok::<_, Failure>(view(context, &details))
                    },
                )
                .map_err(|error| error.0)
        })
        .await
}

pub async fn lorebook_archive(
    context: &ApiContext,
    request: dto::LorebookRevisionRequest,
) -> Result<dto::LorebookView, ApiError> {
    set_status(context, request, true).await
}
pub async fn lorebook_restore(
    context: &ApiContext,
    request: dto::LorebookRevisionRequest,
) -> Result<dto::LorebookView, ApiError> {
    set_status(context, request, false).await
}
async fn set_status(
    context: &ApiContext,
    request: dto::LorebookRevisionRequest,
    archive: bool,
) -> Result<dto::LorebookView, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    if archive {
                        "lorebook_archive"
                    } else {
                        "lorebook_restore"
                    },
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        let details = scope
                            .set_lorebook_status(
                                id,
                                expected,
                                if archive {
                                    LifecycleStatus::Archived
                                } else {
                                    LifecycleStatus::Active
                                },
                                context.now(),
                            )
                            .map_err(|error| Failure(error.into_api_error()))?;
                        Ok::<_, Failure>(view(context, &details))
                    },
                )
                .map_err(|error| error.0)
        })
        .await
}

pub async fn lorebook_delete(
    context: &ApiContext,
    request: dto::LorebookRevisionRequest,
) -> Result<dto::SourceDeleteResult, ApiError> {
    let digest = super::jobs::local::digest(&request)?;
    let id = parse_id(&request.lorebook_id, "lorebook_id")?;
    let expected = revision(request.expected_revision, "expected_revision")?;
    let result = context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "lorebook_delete",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .delete_lorebook(id, expected, context.now())
                            .map(deletion)
                            .map_err(|error| Failure(error.into_api_error()))
                    },
                )
                .map_err(|error| error.0)
        })
        .await?;
    emit_deletion(context, &result);
    context.emit(dto::ApiEvent::LorebooksChanged);
    Ok(result)
}

pub async fn tokens_count(
    context: &ApiContext,
    request: dto::TokensCountRequest,
) -> Result<dto::TokensCount, ApiError> {
    context
        .blocking(move |_| {
            lettuce_context::count_tokens_batch(&request.texts)
                .map(|counts| dto::TokensCount { counts })
                .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))
        })
        .await
}

fn tier(provenance: LorebookSourceProvenance) -> dto::LorebookSourceTier {
    match provenance {
        LorebookSourceProvenance::Character { id } => dto::LorebookSourceTier::Character {
            character_id: id.to_string(),
        },
        LorebookSourceProvenance::Persona { id } => dto::LorebookSourceTier::Persona {
            persona_id: id.to_string(),
        },
        LorebookSourceProvenance::Group { id } => dto::LorebookSourceTier::Group {
            group_id: id.to_string(),
        },
        LorebookSourceProvenance::Starter { character_id, .. } => {
            dto::LorebookSourceTier::Character {
                character_id: character_id.to_string(),
            }
        }
        LorebookSourceProvenance::Conversation { .. } => dto::LorebookSourceTier::Conversation,
    }
}

/// What the next turn injects (conversation mode) or what one book matches
/// in a text (editor mode), from the runtime matcher and in runtime order.
pub async fn lorebook_trigger_preview(
    context: &ApiContext,
    request: dto::LorebookTriggerPreviewRequest,
) -> Result<dto::LorebookTriggerPreview, ApiError> {
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let (entries, skipped, scan_depth, editor) = match request {
                dto::LorebookTriggerPreviewRequest::Conversation {
                    conversation_id,
                    composer_text,
                    speaker_character_id,
                } => {
                    let conversation_id = parse_id(&conversation_id, "conversation_id")?;
                    let speaker = speaker_character_id
                        .as_deref()
                        .map(|id| parse_id::<CharacterId>(id, "speaker_character_id"))
                        .transpose()?;
                    let (lore, scan_depth) =
                        crate::ConversationContextAssembler::new(database)
                            .preview_turn_lore(conversation_id, composer_text.as_deref(), speaker)
                            .map_err(|error| match error {
                                lettuce_conversations::ContextAssemblyError::ConversationUnavailable => {
                                    api_error(ApiErrorCode::NotFound, error.to_string())
                                }
                                error => api_error(ApiErrorCode::Unavailable, error.to_string()),
                            })?;
                    (lore.entries, lore.skipped, scan_depth, false)
                }
                dto::LorebookTriggerPreviewRequest::Editor { lorebook_id, text } => {
                    let id: LorebookId = parse_id(&lorebook_id, "lorebook_id")?;
                    let scan_depth = lettuce_settings::GlobalSettingsStore::load(database)
                        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
                        .settings
                        .lorebook_scan_depth;
                    let details = LorebookRepository::get(database, id)
                        .map_err(IntoApiError::into_api_error)?
                        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "lorebook not found"))?;
                    let mut book = details.book.clone();
                    book.status = LifecycleStatus::Active;
                    let activation = lettuce_context::resolve_lorebook_activation(
                        &[lettuce_context::LorebookActivationSource {
                            provenance: LorebookSourceProvenance::Conversation {
                                id: lettuce_types::ConversationId::from_uuid(uuid::Uuid::nil()),
                            },
                            lorebook_id: id,
                            details: Some(LorebookDetails {
                                book,
                                entries: details.entries,
                            }),
                        }],
                        std::slice::from_ref(&text),
                        Some(text.as_str()),
                        usize::from(scan_depth),
                    )
                    .map_err(|error| invalid_field("lorebook_id", error.to_string()))?;
                    (activation.entries, activation.skipped, scan_depth, true)
                }
            };
            let counts = lettuce_context::count_tokens_batch(
                &entries
                    .iter()
                    .map(|entry| entry.entry.content.trim().to_owned())
                    .collect::<Vec<_>>(),
            )
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
            Ok(dto::LorebookTriggerPreview {
                entries: entries
                    .iter()
                    .zip(counts)
                    .enumerate()
                    .map(|(position, (entry, token_count))| dto::LorebookPreviewEntry {
                        position: u32::try_from(position).unwrap_or(u32::MAX),
                        source: if editor {
                            dto::LorebookSourceTier::Editor
                        } else {
                            tier(entry.source.provenance)
                        },
                        lorebook_id: entry.source.lorebook_id.to_string(),
                        lorebook_name: entry.source.name.clone(),
                        entry_id: entry.entry.id.to_string(),
                        title: entry.entry.title.clone(),
                        matched_keywords: entry.matched_keywords.clone(),
                        always_active: entry.always_active,
                        token_count,
                    })
                    .collect(),
                skipped: skipped
                    .into_iter()
                    .map(|skipped| dto::LorebookSkippedSource {
                        source: tier(skipped.provenance),
                        lorebook_id: skipped.lorebook_id.to_string(),
                        reason: match skipped.reason {
                            LorebookSourceSkipReason::Missing => dto::LorebookSkipReason::Missing,
                            LorebookSourceSkipReason::Archived => {
                                dto::LorebookSkipReason::Archived
                            }
                            LorebookSourceSkipReason::Duplicate => {
                                dto::LorebookSkipReason::Duplicate
                            }
                        },
                    })
                    .collect(),
                scan_depth,
            })
        })
        .await
}

pub(super) fn emit_deletion(context: &ApiContext, result: &dto::SourceDeleteResult) {
    for character_id in &result.character_ids {
        context.emit(dto::ApiEvent::CharacterChanged {
            character_id: character_id.clone(),
        });
    }
    for persona_id in &result.persona_ids {
        context.emit(dto::ApiEvent::PersonaChanged {
            persona_id: persona_id.clone(),
        });
    }
    for group_id in &result.group_ids {
        context.emit(dto::ApiEvent::GroupChanged {
            group_id: group_id.clone(),
        });
    }
    for conversation_id in &result.conversation_ids {
        context.emit(dto::ApiEvent::ConversationChanged {
            conversation_id: conversation_id.clone(),
        });
    }
}
