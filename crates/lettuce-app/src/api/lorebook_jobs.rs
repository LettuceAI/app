use super::jobs::local::{digest, stable_uuid};
use super::{
    ApiContext,
    error::{api_error, invalid_field, parse_id},
};
use lettuce_context::{LifecycleStatus, LorebookRepository, PromptPurpose, PromptRepository};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode, ApiErrorDetails};
use lettuce_models::{
    GlobalModelSettingsRepository, ModelProfileRepository, ProviderAccountRepository,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{ModelProfileId, RequestId};

pub(super) fn failure(error: impl std::error::Error + 'static) -> ApiError {
    use super::error::IntoApiError;
    let mut cause: &(dyn std::error::Error + 'static) = &error;
    loop {
        if let Some(error) = cause.downcast_ref::<lettuce_media::MediaStoreError>() {
            return (*error).into_api_error();
        }
        if let Some(error) = cause.downcast_ref::<crate::StagedLorebookDocumentError>() {
            return ApiError {
                code: match error {
                    crate::StagedLorebookDocumentError::Media(error) => {
                        return (*error).into_api_error();
                    }
                    crate::StagedLorebookDocumentError::Read => ApiErrorCode::Unavailable,
                    _ => ApiErrorCode::InvalidInput,
                },
                message: error.to_string(),
                details: Some(ApiErrorDetails::InvalidField {
                    field: "sources".into(),
                }),
            };
        }
        if let Some(error) = cause.downcast_ref::<lettuce_creation::StagedLorebookSourceError>() {
            return invalid_field("sources", error.to_string());
        }
        if let Some(error) = cause.downcast_ref::<lettuce_jobs::StoreError>() {
            return error.clone().into_api_error();
        }
        if let Some(error) = cause.downcast_ref::<lettuce_context::LorebookRepositoryError>() {
            return error.clone().into_api_error();
        }
        if let Some(error) = cause.downcast_ref::<lettuce_creation::StagedLorebookRepositoryError>()
        {
            return api_error(
                match error {
                    lettuce_creation::StagedLorebookRepositoryError::NotFound => {
                        ApiErrorCode::NotFound
                    }
                    lettuce_creation::StagedLorebookRepositoryError::Conflict => {
                        ApiErrorCode::Conflict
                    }
                    lettuce_creation::StagedLorebookRepositoryError::Invalid => {
                        ApiErrorCode::InvalidInput
                    }
                    _ => ApiErrorCode::Internal,
                },
                error.to_string(),
            );
        }
        if let Some(error) = cause.downcast_ref::<crate::LorebookEntryPreparationError>() {
            if matches!(
                error,
                crate::LorebookEntryPreparationError::InvalidInput
                    | crate::LorebookEntryPreparationError::SourceUnavailable
            ) {
                return api_error(
                    if matches!(
                        error,
                        crate::LorebookEntryPreparationError::SourceUnavailable
                    ) {
                        ApiErrorCode::Unavailable
                    } else {
                        ApiErrorCode::InvalidInput
                    },
                    error.to_string(),
                );
            }
        }
        let invalid = cause
            .downcast_ref::<crate::StagedLorebookAdmissionError>()
            .is_some_and(|error| {
                matches!(error, crate::StagedLorebookAdmissionError::InvalidInput)
            })
            || cause
                .downcast_ref::<crate::StagedLorebookWriterAdmissionError>()
                .is_some_and(|error| {
                    matches!(
                        error,
                        crate::StagedLorebookWriterAdmissionError::InvalidInput
                    )
                })
            || cause
                .downcast_ref::<crate::LorebookKeywordAdmissionError>()
                .is_some_and(|error| {
                    matches!(error, crate::LorebookKeywordAdmissionError::InvalidInput)
                });
        if invalid {
            return api_error(ApiErrorCode::InvalidInput, error.to_string());
        }
        match cause.source() {
            Some(source) => cause = source,
            None => return api_error(ApiErrorCode::Internal, error.to_string()),
        }
    }
}

pub(super) fn model_error(reason: dto::LorebookModelProblem) -> ApiError {
    ApiError {
        code: ApiErrorCode::ModelUnavailable,
        message: "a text model is required".into(),
        details: Some(ApiErrorDetails::LorebookModelUnavailable { reason }),
    }
}

pub(super) fn entry_text_profile(
    context: &ApiContext,
) -> Result<lettuce_conversations::ResolvedInferenceProfile, ApiError> {
    use crate::image::image_feature_models::{
        ImageFeatureModelError, LorebookEntryModelProblem, lorebook_entry_generator_model,
    };
    let db = context.backend().database();
    let settings = GlobalSettingsStore::load(db).map_err(failure)?;
    let model =
        lorebook_entry_generator_model(db, &settings.settings).map_err(|error| match error {
            ImageFeatureModelError::LorebookEntryGenerator(problem) => model_error(match problem {
                LorebookEntryModelProblem::ConfiguredMissing => {
                    dto::LorebookModelProblem::ConfiguredModelMissing
                }
                LorebookEntryModelProblem::ConfiguredNotText => {
                    dto::LorebookModelProblem::ConfiguredModelNotText
                }
                LorebookEntryModelProblem::NoCompatible => dto::LorebookModelProblem::NoTextModel,
            }),
            other => api_error(ApiErrorCode::Unavailable, other.to_string()),
        })?;
    resolved_profile(context, &settings, model.profile, model.account, false)
}

pub(super) fn text_profile(
    context: &ApiContext,
    selected: Option<ModelProfileId>,
) -> Result<lettuce_conversations::ResolvedInferenceProfile, ApiError> {
    let db = context.backend().database();
    let settings = GlobalSettingsStore::load(db).map_err(failure)?;
    let text = |model: &lettuce_models::ModelProfile| {
        model
            .config
            .capabilities
            .input_modalities
            .get(lettuce_models::Modality::Text)
            == lettuce_models::CapabilityStatus::Supported
            && model
                .config
                .capabilities
                .output_modalities
                .get(lettuce_models::Modality::Text)
                == lettuce_models::CapabilityStatus::Supported
    };
    let chosen = selected.or(settings.default_model_profile_id);
    let model = match chosen {
        Some(id) => {
            let model = ModelProfileRepository::get(db, id)
                .map_err(failure)?
                .ok_or_else(|| model_error(dto::LorebookModelProblem::ConfiguredModelMissing))?;
            if !text(&model) {
                return Err(model_error(
                    dto::LorebookModelProblem::ConfiguredModelNotText,
                ));
            }
            model
        }
        None => return Err(model_error(dto::LorebookModelProblem::NoTextModel)),
    };
    let account = ProviderAccountRepository::get(db, model.provider_account_id)
        .map_err(failure)?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "provider account not found"))?;
    resolved_profile(context, &settings, model, account, true)
}

fn resolved_profile(
    context: &ApiContext,
    settings: &lettuce_settings::StoredGlobalSettings,
    model: lettuce_models::ModelProfile,
    account: lettuce_models::ProviderAccount,
    staged: bool,
) -> Result<lettuce_conversations::ResolvedInferenceProfile, ApiError> {
    let db = context.backend().database();
    let slot = if staged {
        &model.config.feature_parameters.lorebook_generator
    } else {
        &model.config.feature_parameters.lorebook_entry_generator
    };
    let parameters = if staged {
        crate::staged_lorebook_parameter_defaults(
            &settings.settings.lorebook_generator,
            slot,
            account.protocol,
            &GlobalModelSettingsRepository::global_model_settings(db)
                .map_err(failure)?
                .0,
        )
    } else {
        crate::feature_parameter_input(
            slot,
            crate::LOREBOOK_ENTRY_GENERATOR_DEFAULTS,
            crate::FeatureRequestFields::Sampling,
            account.protocol,
            &GlobalModelSettingsRepository::global_model_settings(db)
                .map_err(failure)?
                .0,
        )
    };
    let profile = lettuce_models::resolve_chat_profile(
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
        &parameters,
        &Default::default(),
    )
    .map_err(|error| api_error(ApiErrorCode::ModelUnavailable, error.to_string()))?;
    Ok(lettuce_conversations::ResolvedInferenceProfile {
        chat_profile: profile,
        tool_policy: lettuce_conversations::ToolPolicy::Required,
        output_policy: lettuce_conversations::OutputPolicy::Plain,
        safety_policy: lettuce_conversations::SafetyContext::Standard,
        correlation_id: None,
    })
}

pub(super) fn prompt(
    context: &ApiContext,
    id: lettuce_types::PromptDocumentId,
    purpose: PromptPurpose,
) -> Result<lettuce_context::PromptDocument, ApiError> {
    let document = PromptRepository::get(context.backend().database(), id).map_err(failure)?;
    let problem = match document.as_ref() {
        None => Some(dto::ConfiguredPromptProblem::Missing),
        Some(document) if document.status != LifecycleStatus::Active => {
            Some(dto::ConfiguredPromptProblem::Archived)
        }
        Some(document) if document.purpose != purpose => {
            Some(dto::ConfiguredPromptProblem::WrongKind)
        }
        _ => None,
    };
    if let Some(reason) = problem {
        return Err(ApiError {
            code: ApiErrorCode::Unavailable,
            message: "the configured prompt cannot be used".into(),
            details: Some(ApiErrorDetails::ConfiguredPromptUnavailable {
                prompt_id: id.to_string(),
                reason,
            }),
        });
    }
    document.ok_or_else(|| api_error(ApiErrorCode::NotFound, "prompt not found"))
}

pub(super) fn request_id<T: serde::Serialize>(
    command: &str,
    key: &str,
    request: &T,
) -> Result<RequestId, ApiError> {
    super::jobs::local::operation_key(command, key)?;
    let _ = request;
    Ok(RequestId::from_uuid(stable_uuid(&[command, key])))
}

fn fallback(
    settings: &lettuce_settings::LorebookEntryGeneratorSettings,
) -> lettuce_creation::LorebookEntryFallbackFormat {
    match settings.structured_fallback_format {
        lettuce_settings::MemoryStructuredFallbackFormat::Json => {
            lettuce_creation::LorebookEntryFallbackFormat::Json
        }
        lettuce_settings::MemoryStructuredFallbackFormat::Xml => {
            lettuce_creation::LorebookEntryFallbackFormat::Xml
        }
    }
}

pub(super) fn replay(
    context: &ApiContext,
    key: &str,
    digest: &str,
) -> Result<Option<lettuce_types::JobId>, ApiError> {
    match context
        .backend()
        .database()
        .job_operation(key)
        .map_err(failure)?
    {
        Some(operation) if operation.request_digest == digest => Ok(Some(operation.job_id)),
        Some(_) => Err(api_error(
            ApiErrorCode::Conflict,
            "the operation key names another request",
        )),
        None => Ok(None),
    }
}

pub(super) fn with_job_replay(
    context: &ApiContext,
    key: &str,
    digest: &str,
    apply: impl FnOnce() -> Result<lettuce_types::JobId, ApiError>,
) -> Result<lettuce_types::JobId, ApiError> {
    if let Some(id) = replay(context, key, digest)? {
        return Ok(id);
    }
    match apply() {
        Ok(id) => Ok(id),
        Err(error) => replay(context, key, digest)?.ok_or(error),
    }
}

pub(super) fn with_api_replay<T: serde::de::DeserializeOwned>(
    context: &ApiContext,
    command: &str,
    key: &str,
    digest: &str,
    apply: impl FnOnce() -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    let replay = || -> Result<Option<T>, ApiError> {
        let Some(receipt) = context
            .backend()
            .database()
            .lookup_api_operation(command, key)
            .map_err(failure)?
        else {
            return Ok(None);
        };
        if receipt.request_digest != digest {
            return Err(api_error(
                ApiErrorCode::Conflict,
                "the operation key was reused",
            ));
        }
        serde_json::from_value(receipt.result)
            .map(Some)
            .map_err(failure)
    };
    if let Some(value) = replay()? {
        return Ok(value);
    }
    match apply() {
        Ok(value) => Ok(value),
        Err(error) => replay()?.ok_or(error),
    }
}

pub async fn lorebook_entry_draft(
    context: &ApiContext,
    request: dto::LorebookEntryDraftRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let key =
        super::jobs::local::operation_key("lorebook_entry_draft", &request.client_operation_id)?;
    let digest = digest(&request)?;
    let id = request_id(
        "lorebook_entry_draft",
        &request.client_operation_id,
        &request,
    )?;
    let conversation_id = parse_id(&request.conversation_id, "conversation_id")?;
    let lorebook_id = parse_id(&request.lorebook_id, "lorebook_id")?;
    let selected_message_ids = request
        .message_ids
        .iter()
        .map(|id| parse_id(id, "message_ids"))
        .collect::<Result<Vec<_>, _>>()?;
    let selected_memory_ids = request
        .memory_ids
        .iter()
        .map(|id| parse_id(id, "memory_ids"))
        .collect::<Result<Vec<_>, _>>()?;
    let job_id = context
        .blocking(move |context| {
            with_job_replay(context, &key, &digest, || {
                let db = context.backend().database();
                let settings = GlobalSettingsStore::load(db).map_err(failure)?.settings;
                let selected = &settings.lorebook_entry_generator;
                let prompt = prompt(
                    context,
                    selected.entry_prompt_id.unwrap_or(
                        context
                            .backend()
                            .built_in_prompt_ids()
                            .lorebook_entry_writer,
                    ),
                    PromptPurpose::LorebookEntryWriter,
                )?;
                let profile = entry_text_profile(context)?;
                let conversation =
                    lettuce_conversations::ConversationReader::get(db, conversation_id)
                        .map_err(failure)?;
                let clock = crate::companion::companion_clock::companion_clock_context(
                    db,
                    &conversation.conversation,
                )
                .map_err(|error| api_error(ApiErrorCode::Unavailable, format!("{error:?}")))?;

                context
                    .backend()
                    .lorebook_entry_preparation()
                    .with_operation(key.clone(), digest.clone())
                    .prepare_and_admit(crate::LorebookEntryPreparationRequest {
                        request_id: id,
                        conversation_id,
                        lorebook_id,
                        selected_message_ids,
                        selected_memory_ids,
                        source: match request.source {
                            dto::LorebookEntryDraftSource::Messages => {
                                lettuce_creation::LorebookEntrySource::Messages
                            }
                            dto::LorebookEntryDraftSource::Memory => {
                                lettuce_creation::LorebookEntrySource::Memory
                            }
                            dto::LorebookEntryDraftSource::Mixed => {
                                lettuce_creation::LorebookEntrySource::Mixed
                            }
                        },
                        include_memory_summary: request.use_summary,
                        direction_prompt: request.direction,
                        force: request.force,
                        time_awareness_enabled: clock.time_awareness_enabled(),
                        profile,
                        prompt: &prompt,
                        fallback_format: fallback(selected),
                        now: context.now(),
                    })
                    .map(|admission| admission.job.id)
                    .map_err(failure)
            })
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job_id.to_string(),
    })
}

pub async fn lorebook_keywords_draft(
    context: &ApiContext,
    request: dto::LorebookKeywordsDraftRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let key =
        super::jobs::local::operation_key("lorebook_keywords_draft", &request.client_operation_id)?;
    let digest = digest(&request)?;
    let id = request_id(
        "lorebook_keywords_draft",
        &request.client_operation_id,
        &request,
    )?;
    let book = parse_id(&request.lorebook_id, "lorebook_id")?;
    let entry = request
        .entry_id
        .as_deref()
        .map(|id| parse_id::<lettuce_types::LorebookEntryId>(id, "entry_id"))
        .transpose()?;
    let job_id = context
        .blocking(move |context| {
            with_job_replay(context, &key, &digest, || {
                let db = context.backend().database();
                let book = LorebookRepository::get(db, book)
                    .map_err(failure)?
                    .ok_or_else(|| api_error(ApiErrorCode::NotFound, "lorebook not found"))?;
                if entry.is_some_and(|id| !book.entries.iter().any(|entry| entry.id == id)) {
                    return Err(invalid_field(
                        "entry_id",
                        "the entry does not belong to the lorebook",
                    ));
                }
                let selected = GlobalSettingsStore::load(db)
                    .map_err(failure)?
                    .settings
                    .lorebook_entry_generator;
                let prompt = prompt(
                    context,
                    selected.keyword_prompt_id.unwrap_or(
                        context
                            .backend()
                            .built_in_prompt_ids()
                            .lorebook_keyword_generator,
                    ),
                    PromptPurpose::LorebookKeywordGenerator,
                )?;
                let profile = entry_text_profile(context)?;
                context
                    .backend()
                    .lorebook_keyword_coordinator()
                    .with_operation(key.clone(), digest.clone())
                    .prepare_and_admit(crate::LorebookKeywordRequest {
                        request_id: id,
                        title: request.title,
                        content: request.content,
                        direction_prompt: request.direction,
                        existing_keywords: request.existing_keywords,
                        profile,
                        prompt: &prompt,
                        fallback_format: fallback(&selected),
                        now: context.now(),
                    })
                    .map(|admission| admission.job.id)
                    .map_err(failure)
            })
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job_id.to_string(),
    })
}
