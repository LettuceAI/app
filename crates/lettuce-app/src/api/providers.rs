use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_models::{ModelCatalog, ProviderAccount, ProviderAccountRepository, ProviderConfig};
use lettuce_providers::ProviderRequestError;
use lettuce_settings::{
    InMemorySecretStore, SecretOwnerId, SecretPurpose, SecretRecord, SecretRef, SecretState,
    SecretStore, SecretValue,
};
use lettuce_types::{ProviderAccountId, Revision};

use super::ApiContext;
use super::error::{api_error, invalid_field, parse_id};

pub async fn provider_catalog(
    context: &ApiContext,
) -> Result<dto::ProviderCatalogContract, ApiError> {
    let _ = context;
    Ok(crate::generation::provider_runtime::provider_catalog_contract())
}

pub async fn provider_accounts_list(
    context: &ApiContext,
) -> Result<Vec<dto::ProviderAccountView>, ApiError> {
    let accounts = context
        .blocking(|context| {
            context
                .backend()
                .database()
                .provider_accounts()
                .map_err(|_| api_error(ApiErrorCode::Internal, "provider accounts cannot be read"))
        })
        .await?;
    let mut views = Vec::with_capacity(accounts.len());
    for account in accounts {
        let api_key_set = api_key_set(context, &account).await?;
        views.push(account_view(account, api_key_set)?);
    }
    Ok(views)
}

fn remove_secret_references(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            object.remove("api_key_ref");
            for value in object.values_mut() {
                remove_secret_references(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                remove_secret_references(value);
            }
        }
        _ => {}
    }
}

pub(super) async fn account(context: &ApiContext, id: String) -> Result<ProviderAccount, ApiError> {
    let id: ProviderAccountId = parse_id(&id, "account_id")?;
    context
        .blocking(move |context| {
            ProviderAccountRepository::get(context.backend().database(), id)
                .map_err(|_| api_error(ApiErrorCode::Internal, "provider account cannot be read"))?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "provider account does not exist"))
        })
        .await
}

pub(super) fn request_error(error: ProviderRequestError) -> ApiError {
    api_error(
        match error {
            ProviderRequestError::Unsupported => ApiErrorCode::Unsupported,
            ProviderRequestError::Rejected | ProviderRequestError::CredentialRejected => {
                ApiErrorCode::InvalidInput
            }
            ProviderRequestError::Unavailable => ApiErrorCode::Unavailable,
            ProviderRequestError::Malformed => ApiErrorCode::Malformed,
        },
        error.to_string(),
    )
}

fn verified(value: lettuce_providers::KeyVerification) -> Result<dto::ProviderVerified, ApiError> {
    if value.valid {
        return Ok(dto::ProviderVerified {
            valid: true,
            status: value.status,
        });
    }
    Err(ApiError {
        code: ApiErrorCode::InvalidInput,
        message: "provider verification failed".into(),
        details: Some(dto::ApiErrorDetails::ProviderVerification {
            status: value.status,
            provider_message: value.error.unwrap_or_else(|| "Invalid API key".into()),
        }),
    })
}

pub async fn provider_verify(
    context: &ApiContext,
    request: dto::ProviderVerifyRequest,
) -> Result<dto::ProviderVerified, ApiError> {
    match request {
        dto::ProviderVerifyRequest::Saved { account_id } => {
            let account = account(context, account_id).await?;
            verified(
                super::ollama::remote_providers(context)?
                    .verify_api_key(&account)
                    .await
                    .map_err(request_error)?,
            )
        }
        dto::ProviderVerifyRequest::Draft { draft } => {
            let descriptor = lettuce_providers::provider_descriptor(&draft.provider_kind)
                .ok_or_else(|| invalid_field("draft.provider_kind", "unknown provider kind"))?;
            let config: ProviderConfig = serde_json::from_value(draft.config)
                .map_err(|_| invalid_field("draft.config", "invalid provider configuration"))?;
            let now = context.clock().now();
            let owner = SecretOwnerId::new();
            let reference = SecretRef::new();
            let store = Arc::new(InMemorySecretStore::new());
            let key = draft
                .api_key
                .filter(|key| !key.trim().is_empty())
                .map(|key| SecretValue::new(key.trim()))
                .transpose()
                .map_err(|_| invalid_field("draft.api_key", "invalid API key"))?;
            let has_key = key.is_some();
            let account = ProviderAccount {
                id: ProviderAccountId::new(),
                secret_owner_id: owner,
                provider_kind: descriptor.kind.into(),
                protocol: descriptor.protocol,
                label: descriptor.display_name.into(),
                endpoint: draft.base_url,
                enabled: true,
                streaming_enabled: true,
                allow_invalid_tls: false,
                api_key_ref: has_key.then_some(reference),
                secret_headers: Vec::new(),
                config,
                revision: Revision::INITIAL,
                created_at: now,
                updated_at: now,
            };
            lettuce_models::validate_provider_connection(&account)
                .map_err(|_| invalid_field("draft", "invalid provider connection"))?;
            if let Some(key) = key {
                store
                    .put(
                        SecretRecord::new(reference, SecretPurpose::ProviderApiKey { owner }),
                        key,
                        None,
                    )
                    .await
                    .map_err(|_| {
                        api_error(
                            ApiErrorCode::Unavailable,
                            "draft credential cannot be prepared",
                        )
                    })?;
            }
            let tls = context
                .blocking(|context| {
                    context.backend().tls_policy().map_err(|_| {
                        api_error(ApiErrorCode::Unavailable, "TLS settings cannot be read")
                    })
                })
                .await?;
            let network = lettuce_network::JsonClient::with_tls(&tls).map_err(|_| {
                api_error(ApiErrorCode::Unavailable, "provider client cannot be built")
            })?;
            let providers = lettuce_providers::RemoteProviders::new(store, Arc::new(network));
            verified(
                providers
                    .verify_api_key(&account)
                    .await
                    .map_err(request_error)?,
            )
        }
    }
}

pub async fn provider_openrouter_endpoints(
    context: &ApiContext,
    request: dto::ProviderOpenRouterEndpointsRequest,
) -> Result<Vec<dto::ProviderOpenRouterEndpoint>, ApiError> {
    super::ollama::remote_providers(context)?
        .openrouter_public_endpoints(request.model.trim())
        .await
        .map(|endpoints| {
            endpoints
                .into_iter()
                .map(|endpoint| dto::ProviderOpenRouterEndpoint {
                    id: endpoint.id,
                    name: endpoint.name,
                    prompt_price: endpoint.prompt_price,
                    completion_price: endpoint.completion_price,
                    context_length: endpoint.context_length,
                    uptime_last_30m: endpoint.uptime_last_30m,
                    supports_prompt_caching: endpoint.supports_prompt_caching,
                    cache_read_price: endpoint.cache_read_price,
                    cache_write_price: endpoint.cache_write_price,
                })
                .collect()
        })
        .map_err(request_error)
}

pub async fn certificates_list(context: &ApiContext) -> Result<dto::CertificatesView, ApiError> {
    context
        .blocking(|context| {
            let (certificates, revision) = context
                .backend()
                .database()
                .certificates_with_revision()
                .map_err(super::provider_mutations::model_error)?;
            Ok(certificate_view(certificates, revision))
        })
        .await
}

pub(super) fn certificate_view(
    certificates: Vec<lettuce_settings::TrustedCertificate>,
    revision: Revision,
) -> dto::CertificatesView {
    dto::CertificatesView {
        certificates: certificates
            .into_iter()
            .map(|certificate| {
                let valid = lettuce_network::validate_tls_policy(&lettuce_network::TlsPolicy { trusted_roots_pem: vec![certificate.pem.clone()] }).is_ok();
                dto::TrustedCertificateView {
                valid,
                reason: (!valid).then_some(dto::CertificateInvalidReason::InvalidPem),
                id: certificate.id.to_string(),
                name: certificate.name,
                imported_at: certificate.imported_at,
            }})
            .collect(),
        revision: revision.get(),
    }
}

pub async fn provider_models(
    context: &ApiContext,
    request: dto::ProviderModelsRequest,
) -> Result<Vec<dto::RemoteModelContract>, ApiError> {
    let account = account(context, request.account_id).await?;
    super::ollama::remote_providers(context)?
        .list_models(&account)
        .await
        .map(|models| {
            models
                .into_iter()
                .map(|model| dto::RemoteModelContract {
                    id: model.id,
                    display_name: model.display_name,
                    description: model.description,
                    context_length: model.context_length,
                    input_modalities: model.input_modalities,
                    output_modalities: model.output_modalities,
                    supported_endpoints: model.supported_endpoints,
                    input_price: model.input_price,
                    output_price: model.output_price,
                })
                .collect()
        })
        .map_err(request_error)
}

pub async fn provider_model_verify(
    context: &ApiContext,
    request: dto::ProviderModelVerifyRequest,
) -> Result<dto::ProviderModelVerified, ApiError> {
    if request.model.trim().is_empty() {
        return Err(invalid_field("model", "model id is empty"));
    }
    let account = account(context, request.account_id).await?;
    let requested = request.model.trim();
    let requested = if account.protocol == lettuce_models::ProviderProtocol::Gemini {
        requested.strip_prefix("models/").unwrap_or(requested)
    } else {
        requested
    };
    let models = super::ollama::remote_providers(context)?
        .list_models(&account)
        .await
        .map_err(request_error)?;
    Ok(dto::ProviderModelVerified {
        exists: models.iter().any(|model| {
            model.id == requested
                || (account.protocol == lettuce_models::ProviderProtocol::Gemini
                    && model.id.ends_with(&format!("/{requested}")))
        }),
    })
}

pub(super) fn account_view(
    account: ProviderAccount,
    api_key_set: bool,
) -> Result<dto::ProviderAccountView, ApiError> {
    let mut config = serde_json::to_value(&account.config)
        .map_err(|_| api_error(ApiErrorCode::Internal, "provider config cannot be read"))?;
    remove_secret_references(&mut config);
    Ok(dto::ProviderAccountView {
        id: account.id.to_string(),
        provider_kind: account.provider_kind,
        protocol: crate::generation::provider_runtime::protocol_contract(account.protocol),
        label: account.label,
        base_url: account.endpoint,
        enabled: account.enabled,
        streaming_enabled: account.streaming_enabled,
        allow_invalid_tls: account.allow_invalid_tls,
        api_key_set,
        config,
        revision: account.revision.get(),
        created_at: account.created_at.get(),
        updated_at: account.updated_at.get(),
    })
}

pub(super) async fn api_key_set(
    context: &ApiContext,
    account: &ProviderAccount,
) -> Result<bool, ApiError> {
    Ok(match account.api_key_ref {
        Some(reference) => {
            let status = context
                .secret_store()
                .status(
                    &reference,
                    &SecretPurpose::ProviderApiKey {
                        owner: account.secret_owner_id,
                    },
                )
                .await
                .map_err(|_| {
                    api_error(
                        ApiErrorCode::Unavailable,
                        "provider credential status cannot be read",
                    )
                })?;
            match status.state {
                SecretState::Present => true,
                SecretState::Missing => false,
                SecretState::Unavailable { .. } => {
                    return Err(api_error(
                        ApiErrorCode::Unavailable,
                        "provider credential status is unavailable",
                    ));
                }
            }
        }
        None => false,
    })
}

#[cfg(test)]
mod verification_reason_tests {
    #[test]
    fn verification_without_provider_text_has_a_typed_reason() {
        for (status, reason) in [(None, "missing_api_key"), (Some(401), "invalid_api_key")] {
            let error = super::verified(lettuce_providers::KeyVerification { valid: false, status, error: None }).expect_err("invalid");
            let details = serde_json::to_value(error.details).expect("details");
            assert_eq!(details["reason"], reason);
            assert_eq!(details["provider_message"], serde_json::Value::Null);
        }
    }
}
