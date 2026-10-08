use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::ApiOperationError;
use lettuce_models::{
    CapabilityEvidence, CapabilityEvidenceSource, CapabilityStatus, ModalityCapabilities,
    ModelKind, ModelProfile, ModelProfileConfig, ModelProfileRepository,
};
use lettuce_types::{ModelProfileId, Revision};
use serde::{Serialize, de::DeserializeOwned};

fn model_error(error: lettuce_models::ModelRepositoryError) -> ApiError {
    if error == lettuce_models::ModelRepositoryError::AccountMissing {
        api_error(ApiErrorCode::NotFound, "provider account does not exist")
    } else {
        super::provider_mutations::model_error(error)
    }
}
use super::{
    ApiContext,
    error::{api_error, invalid_field, parse_id},
};

struct Failure(ApiError);
impl From<ApiOperationError> for Failure {
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

fn modality(scope: dto::ModelModality) -> lettuce_models::Modality {
    match scope {
        dto::ModelModality::Text => lettuce_models::Modality::Text,
        dto::ModelModality::Image => lettuce_models::Modality::Image,
        dto::ModelModality::Audio => lettuce_models::Modality::Audio,
    }
}

fn declared_view(
    declarations: &Option<Vec<lettuce_models::Modality>>,
    capabilities: &ModalityCapabilities,
) -> Vec<dto::ModelModality> {
    declarations
        .as_ref()
        .map(|scopes| {
            scopes
                .iter()
                .map(|scope| match scope {
                    lettuce_models::Modality::Text => dto::ModelModality::Text,
                    lettuce_models::Modality::Image => dto::ModelModality::Image,
                    lettuce_models::Modality::Audio => dto::ModelModality::Audio,
                })
                .collect()
        })
        .unwrap_or_else(|| scopes(capabilities))
}

fn scopes(capabilities: &ModalityCapabilities) -> Vec<dto::ModelModality> {
    [
        (dto::ModelModality::Text, capabilities.text),
        (dto::ModelModality::Image, capabilities.image),
        (dto::ModelModality::Audio, capabilities.audio),
    ]
    .into_iter()
    .filter_map(|(scope, status)| (status == CapabilityStatus::Supported).then_some(scope))
    .collect()
}

pub(super) fn view(model: ModelProfile) -> Result<dto::ModelView, ApiError> {
    Ok(dto::ModelView {
        id: model.id.to_string(),
        provider_account_id: model.provider_account_id.to_string(),
        external_model_id: model.external_model_id,
        display_name: model.display_name,
        kind: match model.kind {
            ModelKind::Chat => dto::ModelKindContract::Chat,
            ModelKind::Image => dto::ModelKindContract::Image,
            ModelKind::Embedding => dto::ModelKindContract::Embedding,
            ModelKind::Speech => dto::ModelKindContract::Speech,
        },
        input_scopes: declared_view(
            &model.config.capabilities.declared_input_scopes,
            &model.config.capabilities.input_modalities,
        ),
        output_scopes: declared_view(
            &model.config.capabilities.declared_output_scopes,
            &model.config.capabilities.output_modalities,
        ),
        config: serde_json::to_value(model.config)
            .map_err(|_| api_error(ApiErrorCode::Internal, "model config cannot be read"))?,
        revision: model.revision.get(),
        created_at: model.created_at.get(),
        updated_at: model.updated_at.get(),
    })
}

pub async fn models_list(context: &ApiContext) -> Result<dto::ModelsView, ApiError> {
    context
        .blocking(|context| {
            let (models, default, revision) = context
                .backend()
                .database()
                .model_catalog_snapshot()
                .map_err(model_error)?;
            Ok(dto::ModelsView {
                models: models.into_iter().map(view).collect::<Result<_, _>>()?,
                default_model_id: default.map(|id| id.to_string()),
                revision: revision.get(),
            })
        })
        .await
}

pub async fn model_get(
    context: &ApiContext,
    request: dto::ModelGetRequest,
) -> Result<dto::ModelView, ApiError> {
    let id = parse_id(&request.model_id, "model_id")?;
    context
        .blocking(move |context| {
            let model = ModelProfileRepository::get(context.backend().database(), id)
                .map_err(model_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "model does not exist"))?;
            view(model)
        })
        .await
}

fn declared(scopes: &[dto::ModelModality], capabilities: &mut ModalityCapabilities) {
    for status in [
        &mut capabilities.text,
        &mut capabilities.image,
        &mut capabilities.audio,
    ] {
        if *status != CapabilityStatus::Unsupported {
            *status = CapabilityStatus::Unknown;
        }
    }
    for scope in scopes {
        let status = match scope {
            dto::ModelModality::Text => &mut capabilities.text,
            dto::ModelModality::Image => &mut capabilities.image,
            dto::ModelModality::Audio => &mut capabilities.audio,
        };
        if *status != CapabilityStatus::Unsupported {
            *status = CapabilityStatus::Supported;
        }
    }
}

fn metadata(scopes: &[String], capabilities: &mut ModalityCapabilities) {
    for scope in scopes {
        let status = match scope.as_str() {
            "text" => &mut capabilities.text,
            "image" => &mut capabilities.image,
            "audio" => &mut capabilities.audio,
            _ => continue,
        };
        *status = CapabilityStatus::Supported;
    }
}

async fn mutate<T, R>(
    context: &ApiContext,
    command: &'static str,
    request: R,
    key: String,
    apply: impl FnOnce(
        &lettuce_database::ApiOperationTransaction<'_, '_>,
        &ApiContext,
    ) -> Result<T, ApiError>
    + Send
    + 'static,
) -> Result<T, ApiError>
where
    T: Serialize + DeserializeOwned + Send + 'static,
    R: Serialize,
{
    if key.trim().is_empty() {
        return Err(invalid_field(
            "client_operation_id",
            "operation id is empty",
        ));
    }
    let digest = super::jobs::local::digest(&request)?;
    let _guard = context.provider_write_guard().await;
    if context.shutdown_token().is_cancelled() {
        return Err(api_error(
            ApiErrorCode::Cancelled,
            "application is shutting down",
        ));
    }
    context
        .blocking(move |context| {
            if context.shutdown_token().is_cancelled() {
                return Err(api_error(
                    ApiErrorCode::Cancelled,
                    "application is shutting down",
                ));
            }
            context
                .backend()
                .database()
                .commit_api_operation(command, &key, &digest, context.now(), |scope| {
                    apply(scope, context).map_err(Failure)
                })
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn model_save(
    context: &ApiContext,
    request: dto::ModelSaveRequest,
) -> Result<dto::ModelView, ApiError> {
    let copy = request.clone();
    mutate(
        context,
        "model_save",
        copy,
        request.client_operation_id.clone(),
        move |scope, context| {
            let expected = request
                .expected_revision
                .map(|value| super::lorebooks::revision(value, "expected_revision"))
                .transpose()?;
            let id = request
                .model
                .id
                .as_ref()
                .map(|id| parse_id(id, "model.id"))
                .transpose()?
                .unwrap_or_default();
            if request.model.id.is_some() != expected.is_some() {
                return Err(invalid_field(
                    "expected_revision",
                    "existing models require their revision",
                ));
            }
            let mut config: ModelProfileConfig = serde_json::from_value(request.model.config)
                .map_err(|_| invalid_field("model.config", "invalid model configuration"))?;
            let capabilities = &mut config.capabilities;
            let mut remote_used = false;
            if let Some(remote) = request.model.remote_metadata {
                if remote.id != request.model.external_model_id {
                    return Err(invalid_field(
                        "model.remote_metadata",
                        "metadata identity differs from model",
                    ));
                }
                if !remote.input_modalities.as_ref().is_none_or(Vec::is_empty) {
                    metadata(
                        remote.input_modalities.as_deref().unwrap_or_default(),
                        &mut capabilities.input_modalities,
                    );
                    remote_used = true;
                } else {
                    declared(
                        &request.model.input_scopes,
                        &mut capabilities.input_modalities,
                    );
                }
                if !remote.output_modalities.as_ref().is_none_or(Vec::is_empty) {
                    metadata(
                        remote.output_modalities.as_deref().unwrap_or_default(),
                        &mut capabilities.output_modalities,
                    );
                    remote_used = true;
                } else {
                    declared(
                        &request.model.output_scopes,
                        &mut capabilities.output_modalities,
                    );
                }
                if let Some(limit) = remote.context_length {
                    capabilities.context_length = Some(limit.try_into().map_err(|_| {
                        invalid_field(
                            "model.remote_metadata.context_length",
                            "context length is out of range",
                        )
                    })?);
                    remote_used = true;
                }
            } else {
                declared(
                    &request.model.input_scopes,
                    &mut capabilities.input_modalities,
                );
                declared(
                    &request.model.output_scopes,
                    &mut capabilities.output_modalities,
                );
            }
            capabilities.declared_input_scopes = Some(
                request
                    .model
                    .input_scopes
                    .iter()
                    .copied()
                    .map(modality)
                    .collect(),
            );
            capabilities.declared_output_scopes = Some(
                request
                    .model
                    .output_scopes
                    .iter()
                    .copied()
                    .map(modality)
                    .collect(),
            );
            capabilities.evidence = CapabilityEvidence {
                source: if remote_used {
                    CapabilityEvidenceSource::ProviderReported
                } else {
                    CapabilityEvidenceSource::UserOverride
                },
                source_version: 1,
                observed_at: context.now(),
            };
            let model = ModelProfile {
                id,
                provider_account_id: parse_id(
                    &request.model.provider_account_id,
                    "model.provider_account_id",
                )?,
                external_model_id: request.model.external_model_id,
                display_name: request.model.display_name,
                kind: match request.model.kind {
                    dto::ModelKindContract::Chat => ModelKind::Chat,
                    dto::ModelKindContract::Image => ModelKind::Image,
                    dto::ModelKindContract::Embedding => ModelKind::Embedding,
                    dto::ModelKindContract::Speech => ModelKind::Speech,
                },
                config,
                revision: Revision::INITIAL,
                created_at: context.now(),
                updated_at: context.now(),
            };
            view(
                scope
                    .save_model_profile(model, expected, true)
                    .map_err(model_error)?,
            )
        },
    )
    .await
}

pub async fn model_duplicate(
    context: &ApiContext,
    request: dto::ModelDuplicateRequest,
) -> Result<dto::ModelView, ApiError> {
    let copy = request.clone();
    mutate(
        context,
        "model_duplicate",
        copy,
        request.client_operation_id.clone(),
        move |scope, context| {
            if request.display_name.trim().is_empty() {
                return Err(invalid_field("display_name", "display name is empty"));
            }
            let id: ModelProfileId = parse_id(&request.model_id, "model_id")?;
            let expected =
                super::lorebooks::revision(request.expected_revision, "expected_revision")?;
            let mut model = scope
                .model_profile(id)
                .map_err(model_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "model does not exist"))?;
            if model.revision != expected {
                return Err(api_error(ApiErrorCode::Conflict, "model revision changed"));
            }
            model.id = ModelProfileId::new();
            model.display_name = request.display_name;
            model.revision = Revision::INITIAL;
            model.created_at = context.now();
            model.updated_at = context.now();
            view(
                scope
                    .save_model_profile(model, None, false)
                    .map_err(model_error)?,
            )
        },
    )
    .await
}

pub async fn model_delete(
    context: &ApiContext,
    request: dto::ModelDeleteRequest,
) -> Result<(), ApiError> {
    let copy = request.clone();
    let (characters, groups) = mutate(
        context,
        "model_delete",
        copy,
        request.client_operation_id.clone(),
        move |scope, context| {
            let id = parse_id(&request.model_id, "model_id")?;
            let expected =
                super::lorebooks::revision(request.expected_revision, "expected_revision")?;
            scope
                .delete_model_profile(id, expected, context.now())
                .map_err(model_error)
        },
    )
    .await?;
    for character_id in characters {
        context.emit(dto::ApiEvent::CharacterChanged { character_id });
    }
    for group_id in groups {
        context.emit(dto::ApiEvent::GroupChanged { group_id });
    }
    context.emit(dto::ApiEvent::SettingsChanged {
        section: "models".into(),
    });
    Ok(())
}

pub async fn model_default_set(
    context: &ApiContext,
    request: dto::ModelDefaultSetRequest,
) -> Result<dto::ModelDefaultView, ApiError> {
    let copy = request.clone();
    mutate(
        context,
        "model_default_set",
        copy,
        request.client_operation_id.clone(),
        move |scope, context| {
            let id = request
                .model_id
                .as_ref()
                .map(|id| parse_id(id, "model_id"))
                .transpose()?;
            let expected =
                super::lorebooks::revision(request.expected_revision, "expected_revision")?;
            let revision = scope
                .set_default_model_profile(id, expected, context.now())
                .map_err(model_error)?;
            Ok(dto::ModelDefaultView {
                model_id: id.map(|id| id.to_string()),
                revision: revision.get(),
            })
        },
    )
    .await
}
