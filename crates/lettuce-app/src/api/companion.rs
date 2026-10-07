use lettuce_characters::{CharacterDetails, CharacterRepository, InteractionMode};
use lettuce_companions::{
    CompanionScheduledNote, CompanionScheduledNoteError, CompanionScheduledNoteRepository,
    CompanionSoulConfig, ScheduledNoteRecurrence, SoulFact, SoulOwner, SoulRepository,
    SoulRepositoryError,
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{ConversationKind, ConversationLifecycle, ConversationReader};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{CharacterId, ConversationId, ModelProfileId, RequestId, TimestampMillis};
use uuid::Uuid;

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::jobs::local::stable_uuid;
use super::memory_control::controlled;

fn soul_error(error: SoulRepositoryError) -> ApiError {
    api_error(
        match error {
            SoulRepositoryError::NotFound => ApiErrorCode::NotFound,
            SoulRepositoryError::Conflict
            | SoulRepositoryError::AlreadyExists
            | SoulRepositoryError::OperationMismatch => ApiErrorCode::Conflict,
            SoulRepositoryError::Invalid(_)
            | SoulRepositoryError::Corrupt
            | SoulRepositoryError::Failure => ApiErrorCode::Internal,
        },
        format!("the Soul could not be updated: {error:?}"),
    )
}

fn note_error(error: CompanionScheduledNoteError) -> ApiError {
    api_error(
        match error {
            CompanionScheduledNoteError::NotFound
            | CompanionScheduledNoteError::CharacterNotFound => ApiErrorCode::NotFound,
            CompanionScheduledNoteError::NotCompanion => ApiErrorCode::Unsupported,
            CompanionScheduledNoteError::Invalid => ApiErrorCode::InvalidInput,
            CompanionScheduledNoteError::Conflict => ApiErrorCode::Conflict,
            CompanionScheduledNoteError::Failure | CompanionScheduledNoteError::Corrupt => {
                ApiErrorCode::Internal
            }
        },
        error.to_string(),
    )
}

fn companion_character(
    database: &lettuce_database::Database,
    id: CharacterId,
) -> Result<(CharacterDetails, CompanionSoulConfig), ApiError> {
    let details = CharacterRepository::get(database, id)
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the character was not found"))?;
    if details.character.defaults.interaction_mode != InteractionMode::Companion {
        return Err(api_error(
            ApiErrorCode::Unsupported,
            "the character is not a companion",
        ));
    }
    let config = details
        .character
        .defaults
        .companion_soul
        .clone()
        .unwrap_or_default();
    Ok((details, config))
}

struct Target {
    character_id: CharacterId,
    conversation_id: Option<ConversationId>,
    owner: SoulOwner,
    config: CompanionSoulConfig,
}

fn target(
    database: &lettuce_database::Database,
    character_id: &str,
    conversation_id: Option<&str>,
) -> Result<Target, ApiError> {
    let character_id: CharacterId = parse_id(character_id, "character_id")?;
    let (_, config) = companion_character(database, character_id)?;
    let conversation_id: Option<ConversationId> = conversation_id
        .map(|value| parse_id(value, "conversation_id"))
        .transpose()?;
    let owner = match conversation_id {
        None => SoulOwner::Character(character_id),
        Some(conversation_id) => {
            let aggregate = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            if aggregate.conversation.lifecycle == ConversationLifecycle::Tombstoned {
                return Err(api_error(
                    ApiErrorCode::NotFound,
                    "the conversation was not found",
                ));
            }
            let ConversationKind::Direct(details) = &aggregate.conversation.kind else {
                return Err(invalid_field(
                    "conversation_id",
                    "the conversation is not a chat with this character",
                ));
            };
            if details.character.source_id != character_id {
                return Err(invalid_field(
                    "conversation_id",
                    "the conversation is not a chat with this character",
                ));
            }
            SoulOwner::for_conversation(
                character_id,
                conversation_id,
                config.share_soul_growth_across_chats,
            )
        }
    };
    Ok(Target {
        character_id,
        conversation_id,
        owner,
        config,
    })
}

fn category(value: lettuce_companions::SoulCategory) -> dto::SoulCategory {
    use dto::SoulCategory as D;
    use lettuce_companions::SoulCategory as S;
    match value {
        S::Essence => D::Essence,
        S::Traits => D::Traits,
        S::Backstory => D::Backstory,
        S::Appearance => D::Appearance,
        S::Goals => D::Goals,
        S::Likes => D::Likes,
        S::Voice => D::Voice,
        S::RelationalStyle => D::RelationalStyle,
        S::Vulnerabilities => D::Vulnerabilities,
        S::Fears => D::Fears,
        S::Habits => D::Habits,
        S::Boundaries => D::Boundaries,
    }
}

fn fact_view(fact: &SoulFact) -> dto::SoulFactView {
    dto::SoulFactView {
        id: fact.id.clone(),
        category: category(fact.category),
        value: fact.value.clone(),
        kind: match fact.kind {
            lettuce_companions::SoulFactKind::Add => dto::SoulFactKind::Add,
            lettuce_companions::SoulFactKind::Adjust => dto::SoulFactKind::Adjust,
            lettuce_companions::SoulFactKind::Authored => dto::SoulFactKind::Authored,
            lettuce_companions::SoulFactKind::Consolidated => dto::SoulFactKind::Consolidated,
        },
        policy: match fact.policy {
            lettuce_companions::SoulFactPolicy::Current => dto::SoulFactPolicy::Current,
            lettuce_companions::SoulFactPolicy::Adaptive => dto::SoulFactPolicy::Adaptive,
            lettuce_companions::SoulFactPolicy::Historical => dto::SoulFactPolicy::Historical,
        },
        slot: fact.slot.clone(),
        confidence: fact.confidence,
        evidence_count: fact.evidence_count,
        weight: fact.weight,
        valid_from: fact.valid_from.get(),
        valid_until: fact.valid_until.map(TimestampMillis::get),
        locked: fact.locked,
        created_at: fact.created_at.get(),
        superseded_by: fact.superseded_by.clone(),
        superseded_at: fact.superseded_at.map(TimestampMillis::get),
    }
}

/// The companion's authored Soul configuration together with the growth of
/// the Soul in effect: the conversation's own while its character does not
/// share Soul growth, otherwise the character's.
pub async fn companion_soul_get(
    context: &ApiContext,
    request: dto::CompanionSoulGetRequest,
) -> Result<dto::CompanionSoulView, ApiError> {
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let target = target(
                database,
                &request.character_id,
                request.conversation_id.as_deref(),
            )?;
            let state = SoulRepository::get(database, target.owner).map_err(soul_error)?;
            let facts = state
                .as_ref()
                .map(|state| state.facts.iter().map(fact_view).collect::<Vec<_>>())
                .unwrap_or_default();
            let superseded = facts
                .iter()
                .filter(|fact| fact.superseded_by.is_some())
                .count();
            Ok(dto::CompanionSoulView {
                character_id: target.character_id.to_string(),
                conversation_id: target.conversation_id.map(|id| id.to_string()),
                config: serde_json::to_value(&target.config).map_err(|_| {
                    api_error(
                        ApiErrorCode::Internal,
                        "the Soul configuration could not be encoded",
                    )
                })?,
                growth: dto::SoulGrowthView {
                    owner: match target.owner {
                        SoulOwner::Character(_) => dto::SoulOwnerKind::Character,
                        SoulOwner::Conversation { .. } => dto::SoulOwnerKind::Conversation,
                    },
                    revision: state.as_ref().map_or(0, |state| state.revision.get()),
                    active_count: u32::try_from(facts.len() - superseded).unwrap_or(u32::MAX),
                    superseded_count: u32::try_from(superseded).unwrap_or(u32::MAX),
                    facts,
                },
            })
        })
        .await
}

/// Removes every growth entry, locked ones included, and answers how many
/// there were.
pub async fn companion_soul_growth_clear(
    context: &ApiContext,
    request: dto::CompanionSoulGrowthClearRequest,
) -> Result<u32, ApiError> {
    let key = request.client_operation_id.clone();
    let (character, conversation) = (
        request.character_id.clone(),
        request.conversation_id.clone(),
    );
    controlled(
        context,
        "companion_soul_growth_clear",
        &key,
        &request,
        move |context| {
            let database = context.backend().database();
            let target = target(database, &character, conversation.as_deref())?;
            crate::clear_companion_soul_growth(database, target.owner, context.now())
                .map_err(soul_error)
        },
    )
    .await
}

/// Removes one growth entry by its id, even a locked one; false when there
/// is none with that id.
pub async fn companion_soul_growth_remove(
    context: &ApiContext,
    request: dto::CompanionSoulGrowthRemoveRequest,
) -> Result<bool, ApiError> {
    let key = request.client_operation_id.clone();
    let (character, conversation, fact) = (
        request.character_id.clone(),
        request.conversation_id.clone(),
        request.fact_id.clone(),
    );
    controlled(
        context,
        "companion_soul_growth_remove",
        &key,
        &request,
        move |context| {
            let database = context.backend().database();
            let target = target(database, &character, conversation.as_deref())?;
            crate::remove_companion_soul_growth(database, target.owner, &fact, context.now())
                .map_err(soul_error)
        },
    )
    .await
}

/// Locks or unlocks one growth entry; true when the entry exists, whether or
/// not the lock changed.
pub async fn companion_soul_growth_lock(
    context: &ApiContext,
    request: dto::CompanionSoulGrowthLockRequest,
) -> Result<bool, ApiError> {
    let key = request.client_operation_id.clone();
    let (character, conversation, fact, locked) = (
        request.character_id.clone(),
        request.conversation_id.clone(),
        request.fact_id.clone(),
        request.locked,
    );
    controlled(
        context,
        "companion_soul_growth_lock",
        &key,
        &request,
        move |context| {
            let database = context.backend().database();
            let target = target(database, &character, conversation.as_deref())?;
            crate::set_companion_soul_growth_lock(
                database,
                target.owner,
                &fact,
                locked,
                context.now(),
            )
            .map_err(soul_error)
        },
    )
    .await
}

fn writer_profile(
    database: &lettuce_database::Database,
    model_profile_id: ModelProfileId,
) -> Result<lettuce_conversations::ResolvedInferenceProfile, ApiError> {
    let model = lettuce_models::ModelProfileRepository::get(database, model_profile_id)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .ok_or_else(|| invalid_field("model_profile_id", "the model was not found"))?;
    let account =
        lettuce_models::ProviderAccountRepository::get(database, model.provider_account_id)
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
            .ok_or_else(|| {
                invalid_field("model_profile_id", "the model's account was not found")
            })?;
    let global = lettuce_models::GlobalModelSettingsRepository::global_model_settings(database)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .0;
    let chat_profile = lettuce_models::resolve_chat_profile(
        &lettuce_models::ExpectedModelIdentity {
            model_profile_id: model.id,
            model_revision: model.revision,
            provider_account_id: account.id,
            provider_account_revision: account.revision,
            external_model_id: model.external_model_id.clone(),
            display_name: model.display_name.clone(),
            provider_protocol: account.protocol,
            model_kind: model.kind,
        },
        &model,
        &account,
        &crate::feature_parameter_input(
            &model.config.feature_parameters.companion_soul_writer,
            crate::COMPANION_SOUL_WRITER_DEFAULTS,
            crate::FeatureRequestFields::Sampling,
            account.protocol,
            &global,
        ),
        &lettuce_models::ChatRequirements::default(),
    )
    .map_err(|_| invalid_field("model_profile_id", "the model cannot write a Soul"))?;
    Ok(lettuce_conversations::ResolvedInferenceProfile {
        chat_profile,
        tool_policy: lettuce_conversations::ToolPolicy::Required,
        output_policy: lettuce_conversations::OutputPolicy::Plain,
        safety_policy: lettuce_conversations::SafetyContext::Standard,
        correlation_id: None,
    })
}

/// Queues a Soul writer run over the unsaved draft. The model is the chosen
/// one, then the configured Soul writer model, then the default model; the
/// configured fallback model runs only when it differs. The draft arrives as
/// the job's result and nothing is saved.
pub async fn companion_soul_writer_run(
    context: &ApiContext,
    request: dto::CompanionSoulWriterRunRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let key = request.client_operation_id.clone();
    let action_key = key.clone();
    let action_request = request.clone();
    let accepted = controlled(
        context,
        "companion_soul_writer_run",
        &key,
        &request,
        move |context| {
            let request = action_request;
            if request.character_name.trim().is_empty() {
                return Err(invalid_field("character_name", "the character needs a name"));
            }
            let database = context.backend().database();
            let stored = GlobalSettingsStore::load(database)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            let configured = stored.settings.companion_soul_writer;
            let chosen: Option<ModelProfileId> = request
                .model_profile_id
                .as_deref()
                .map(|value| parse_id(value, "model_profile_id"))
                .transpose()?;
            let primary_id = chosen
                .or(configured.model_profile_id)
                .or(stored.default_model_profile_id)
                .ok_or_else(|| {
                    invalid_field(
                        "model_profile_id",
                        "no model is available for the Soul writer",
                    )
                })?;
            let primary = writer_profile(database, primary_id)?;
            let fallback = configured
                .fallback_model_profile_id
                .filter(|id| *id != primary_id)
                .and_then(|id| match writer_profile(database, id) {
                    Ok(profile) => Some(profile),
                    Err(error) => {
                        tracing::warn!(reason = %error.message, "the Soul writer fallback model cannot run");
                        None
                    }
                });
            let prompt = match configured.prompt_id.and_then(|id| {
                lettuce_context::PromptRepository::get(database, id)
                    .ok()
                    .flatten()
                    .filter(|document| {
                        document.status == lettuce_context::LifecycleStatus::Active
                            && document.purpose
                                == lettuce_context::PromptPurpose::CompanionSoulWriter
                    })
            }) {
                Some(prompt) => prompt,
                None => crate::generation::built_in_prompts::active_built_in_prompt(
                    database,
                    crate::BuiltInPromptId::CompanionSoulWriter,
                )
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
                .ok_or_else(|| {
                    api_error(ApiErrorCode::Internal, "the Soul writer prompt is missing")
                })?,
            };
            let request_id =
                RequestId::from_uuid(stable_uuid(&["companion-soul-writer", &action_key]));
            let admission = context
                .backend()
                .companion_soul_writer_admission()
                .admit(crate::CompanionSoulWriterAdmissionRequest {
                    request_id,
                    primary_profile: primary,
                    fallback_profile: fallback,
                    prompt: &prompt,
                    character_name: request.character_name.trim(),
                    character_definition: request.character_definition.as_deref(),
                    character_description: request.character_description.as_deref(),
                    opening_context: request.opening_context.as_deref(),
                    current_soul: request.current_soul.as_ref(),
                    user_notes: request.user_notes.as_deref(),
                    fallback_format: match configured.structured_fallback_format {
                        lettuce_settings::MemoryStructuredFallbackFormat::Json => {
                            lettuce_companions::SoulWriterFallbackFormat::Json
                        }
                        lettuce_settings::MemoryStructuredFallbackFormat::Xml => {
                            lettuce_companions::SoulWriterFallbackFormat::Xml
                        }
                    },
                    now: context.now(),
                })
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            Ok(dto::JobAccepted {
                job_id: admission.job.id.to_string(),
            })
        },
    )
    .await?;
    context.jobs().wake();
    Ok(accepted)
}

fn recurrence(value: dto::CompanionNoteRecurrence) -> ScheduledNoteRecurrence {
    match value {
        dto::CompanionNoteRecurrence::None => ScheduledNoteRecurrence::None,
        dto::CompanionNoteRecurrence::Daily => ScheduledNoteRecurrence::Daily,
        dto::CompanionNoteRecurrence::Weekly => ScheduledNoteRecurrence::Weekly,
        dto::CompanionNoteRecurrence::Monthly => ScheduledNoteRecurrence::Monthly,
        dto::CompanionNoteRecurrence::Yearly => ScheduledNoteRecurrence::Yearly,
    }
}

fn note_view(note: &CompanionScheduledNote) -> dto::CompanionNoteView {
    dto::CompanionNoteView {
        id: note.id.to_string(),
        character_id: note.character_id.to_string(),
        label: note.label.clone(),
        content: note.content.clone(),
        available_at: note.available_at.get(),
        expires_at: note.expires_at.map(TimestampMillis::get),
        recurrence: match note.recurrence {
            ScheduledNoteRecurrence::None => dto::CompanionNoteRecurrence::None,
            ScheduledNoteRecurrence::Daily => dto::CompanionNoteRecurrence::Daily,
            ScheduledNoteRecurrence::Weekly => dto::CompanionNoteRecurrence::Weekly,
            ScheduledNoteRecurrence::Monthly => dto::CompanionNoteRecurrence::Monthly,
            ScheduledNoteRecurrence::Yearly => dto::CompanionNoteRecurrence::Yearly,
        },
        recurrence_window_ms: note.recurrence_window_ms,
        enabled: note.enabled,
        created_at: note.created_at.get(),
        updated_at: note.updated_at.get(),
    }
}

/// The character's scheduled notes in the order they become available.
pub async fn companion_notes_list(
    context: &ApiContext,
    request: dto::CompanionNotesRequest,
) -> Result<Vec<dto::CompanionNoteView>, ApiError> {
    let character_id: CharacterId = parse_id(&request.character_id, "character_id")?;
    context
        .blocking(move |context| {
            CompanionScheduledNoteRepository::list_scheduled_notes(
                context.backend().database(),
                character_id,
            )
            .map(|notes| notes.iter().map(note_view).collect())
            .map_err(note_error)
        })
        .await
}

/// Creates a note, or updates the one `note_id` names. The API assigns the
/// id and the timestamps: a retry of the same request keeps the id it first
/// assigned, and an update keeps the note's creation time.
pub async fn companion_notes_upsert(
    context: &ApiContext,
    request: dto::CompanionNoteUpsertRequest,
) -> Result<dto::CompanionNoteView, ApiError> {
    let character_id: CharacterId = parse_id(&request.character_id, "character_id")?;
    let note_id: Option<Uuid> = request
        .note_id
        .as_deref()
        .map(|value| parse_id(value, "note_id"))
        .transpose()?;
    if request.content.trim().is_empty() {
        return Err(invalid_field("content", "the note needs content"));
    }
    if request.available_at < 0 {
        return Err(invalid_field("available_at", "the start time is not valid"));
    }
    if request
        .expires_at
        .is_some_and(|expires| expires <= request.available_at)
    {
        return Err(invalid_field(
            "expires_at",
            "the end must be after the start",
        ));
    }
    let key = request.client_operation_id.clone();
    let action_key = key.clone();
    let action_request = request.clone();
    controlled(
        context,
        "companion_notes_upsert",
        &key,
        &request,
        move |context| {
            let request = action_request;
            let database = context.backend().database();
            let existing =
                CompanionScheduledNoteRepository::list_scheduled_notes(database, character_id)
                    .map_err(note_error)?;
            let id = match note_id {
                Some(id) => {
                    if existing.iter().all(|note| note.id != id) {
                        return Err(api_error(ApiErrorCode::NotFound, "the note was not found"));
                    }
                    id
                }
                None => stable_uuid(&["companion-scheduled-note", &action_key]),
            };
            let now = context.now();
            let created_at = existing
                .iter()
                .find(|note| note.id == id)
                .map_or(now, |note| note.created_at);
            let stored = CompanionScheduledNoteRepository::upsert_scheduled_note(
                database,
                CompanionScheduledNote {
                    id,
                    character_id,
                    label: request.label,
                    content: request.content,
                    available_at: TimestampMillis::new(request.available_at),
                    expires_at: request.expires_at.map(TimestampMillis::new),
                    recurrence: recurrence(request.recurrence),
                    recurrence_window_ms: request.recurrence_window_ms,
                    enabled: request.enabled,
                    created_at,
                    updated_at: now,
                },
            )
            .map_err(note_error)?;
            Ok(note_view(&stored))
        },
    )
    .await
}

/// Deletes a note; a note that does not exist is not an error.
pub async fn companion_notes_delete(
    context: &ApiContext,
    request: dto::CompanionNoteDeleteRequest,
) -> Result<(), ApiError> {
    let note_id: Uuid = parse_id(&request.note_id, "note_id")?;
    let key = request.client_operation_id.clone();
    controlled(
        context,
        "companion_notes_delete",
        &key,
        &request,
        move |context| {
            CompanionScheduledNoteRepository::delete_scheduled_note(
                context.backend().database(),
                note_id,
            )
            .map_err(note_error)
        },
    )
    .await
}

/// The enabled notes that apply at `as_of`.
pub async fn companion_notes_active_preview(
    context: &ApiContext,
    request: dto::CompanionNotesActivePreviewRequest,
) -> Result<Vec<dto::CompanionNoteView>, ApiError> {
    let character_id: CharacterId = parse_id(&request.character_id, "character_id")?;
    if request.as_of < 0 {
        return Err(invalid_field("as_of", "the time is not valid"));
    }
    context
        .blocking(move |context| {
            let notes = CompanionScheduledNoteRepository::list_scheduled_notes(
                context.backend().database(),
                character_id,
            )
            .map_err(note_error)?;
            lettuce_companions::active_scheduled_notes(notes, TimestampMillis::new(request.as_of))
                .map(|notes| notes.iter().map(note_view).collect())
                .map_err(note_error)
        })
        .await
}
