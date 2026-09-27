//! An Ollama server's own model store: what it has, deleting a model and
//! pulling one (a job).

use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_models::{ProviderAccount, ProviderAccountRepository};
use lettuce_providers::{OllamaHubError, RemoteProviders};
use lettuce_settings::SecretStore;
use lettuce_types::ProviderAccountId;

use super::ApiContext;
use super::error::{api_error, invalid_field, parse_id};

pub(crate) fn ollama_error(error: OllamaHubError) -> ApiError {
    match error {
        OllamaHubError::NotOllama => invalid_field("provider_account_id", error.to_string()),
        OllamaHubError::EmptyReference => invalid_field("model", error.to_string()),
        OllamaHubError::Credentials | OllamaHubError::Incomplete | OllamaHubError::Message(_) => {
            api_error(ApiErrorCode::Unavailable, error.to_string())
        }
    }
}

/// The Ollama account `id` names.
pub(crate) async fn ollama_account(
    context: &ApiContext,
    id: &str,
) -> Result<ProviderAccount, ApiError> {
    let id: ProviderAccountId = parse_id(id, "provider_account_id")?;
    let account = context
        .blocking(move |context| {
            ProviderAccountRepository::get(context.backend().database(), id)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
        })
        .await?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the account was not found"))?;
    if account.protocol != lettuce_models::ProviderProtocol::Ollama
        || !account.provider_kind.eq_ignore_ascii_case("ollama")
    {
        return Err(ollama_error(OllamaHubError::NotOllama));
    }
    Ok(account)
}

/// The provider operations over the host's secrets and trusted certificates.
pub(crate) fn remote_providers(
    context: &ApiContext,
) -> Result<RemoteProviders<dyn SecretStore>, ApiError> {
    let tls = context
        .backend()
        .tls_policy()
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    let network = lettuce_network::JsonClient::with_tls(&tls)
        .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
    Ok(RemoteProviders::new(
        Arc::clone(context.secret_store()),
        Arc::new(network),
    ))
}

pub async fn ollama_models_list(
    context: &ApiContext,
    request: dto::OllamaModelsRequest,
) -> Result<dto::OllamaModelList, ApiError> {
    let account = ollama_account(context, &request.provider_account_id).await?;
    let models = remote_providers(context)?
        .ollama_inventory(&account)
        .await
        .map_err(ollama_error)?;
    Ok(dto::OllamaModelList {
        models: models
            .into_iter()
            .map(|model| dto::OllamaModel {
                name: model.name,
                size: model.size,
                modified_at: model.modified_at,
                digest: model.digest,
                parameter_size: model.parameter_size,
                quantization_level: model.quantization_level,
                family: model.family,
            })
            .collect(),
    })
}

pub async fn ollama_model_delete(
    context: &ApiContext,
    request: dto::OllamaModelDeleteRequest,
) -> Result<(), ApiError> {
    let model = request.model.trim();
    if model.is_empty() {
        return Err(invalid_field("model", "model is empty"));
    }
    let account = ollama_account(context, &request.provider_account_id).await?;
    remote_providers(context)?
        .ollama_delete(&account, model)
        .await
        .map_err(ollama_error)
}

/// Pulls a model into the account's Ollama server as a job; a pull of the
/// same model already queued or running is joined, and a retried
/// `client_operation_id` returns its job.
pub async fn ollama_pull(
    context: &ApiContext,
    request: dto::OllamaPullRequest,
) -> Result<dto::JobAccepted, ApiError> {
    super::jobs::admit_model_pull(context, request).await
}
