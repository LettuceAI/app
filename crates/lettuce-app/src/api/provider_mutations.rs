use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::ApiOperationError;
use lettuce_models::{ModelRepositoryError, ProviderAccount, ProviderConfig};
use lettuce_settings::{
    SecretOwnerId, SecretPurpose, SecretRecord, SecretRef, SecretValue, TrustedCertificate,
};
use lettuce_types::{ProviderAccountId, Revision};
use serde::{Serialize, de::DeserializeOwned};

use super::{
    ApiContext,
    error::{IntoApiError, api_error, invalid_field, parse_id},
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
pub(super) fn model_error(error: ModelRepositoryError) -> ApiError {
    let code = match &error {
        ModelRepositoryError::NotFound => ApiErrorCode::NotFound,
        ModelRepositoryError::StaleRevision | ModelRepositoryError::AlreadyExists => {
            ApiErrorCode::Conflict
        }
        ModelRepositoryError::AccountInUse(_) | ModelRepositoryError::InUse(_) => {
            ApiErrorCode::InUse
        }
        ModelRepositoryError::InvalidData => ApiErrorCode::InvalidInput,
        _ => ApiErrorCode::Internal,
    };
    let details = if let ModelRepositoryError::AccountInUse(models) = &error {
        Some(dto::ApiErrorDetails::ProviderModelsInUse {
            models: models.iter().map(ToString::to_string).collect(),
        })
    } else {
        None
    };
    ApiError {
        code,
        message: error.to_string(),
        details,
    }
}
fn digest<T: Serialize>(request: &T, key: &str) -> Result<String, ApiError> {
    if key.trim().is_empty() {
        return Err(invalid_field(
            "client_operation_id",
            "operation id is empty",
        ));
    }
    super::jobs::local::digest(request)
}
fn identity(kind: &str, key: &str, digest: &str) -> uuid::Uuid {
    uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        format!("lettuce-provider:{kind}:{key}:{digest}").as_bytes(),
    )
}
async fn replay<T: DeserializeOwned + Send + 'static>(
    context: &ApiContext,
    command: &'static str,
    key: String,
    digest: String,
) -> Result<Option<T>, ApiError> {
    context
        .blocking(move |context| {
            let receipt = context
                .backend()
                .database()
                .lookup_api_operation(command, &key)
                .map_err(|error| Failure::from(error).0)?;
            receipt
                .map(|receipt| {
                    if receipt.request_digest != digest {
                        return Err(api_error(
                            ApiErrorCode::Conflict,
                            "operation id was already used with another request",
                        ));
                    }
                    serde_json::from_value(receipt.result).map_err(|_| {
                        api_error(ApiErrorCode::Internal, "operation receipt cannot be read")
                    })
                })
                .transpose()
        })
        .await
}
pub(super) async fn cleanup_secrets(context: &ApiContext, staged: bool) -> Result<(), ApiError> {
    if let Err(error) = cleanup_secrets_inner(context, staged).await {
        tracing::warn!(code = ?error.code, "provider credential cleanup deferred");
    }
    Ok(())
}

async fn cleanup_secrets_inner(context: &ApiContext, staged: bool) -> Result<(), ApiError> {
    let _guard = if staged {
        Some(context.provider_write_guard().await)
    } else {
        None
    };
    let records = context
        .blocking(move |context| {
            let database = context.backend().database();
            let records = database.provider_secret_cleanup().map_err(model_error)?;
            if staged {
                return Ok(records);
            }
            records
                .into_iter()
                .filter_map(
                    |record| match database.provider_secret_is_retired(&record) {
                        Ok(true) => Some(Ok(record)),
                        Ok(false) => None,
                        Err(error) => Some(Err(model_error(error))),
                    },
                )
                .collect()
        })
        .await?;
    for record in records {
        if context.secret_store().delete(&record.reference, &record.purpose, None).await.is_err() {
            tracing::warn!("provider credential deletion deferred");
            continue;
        }
        context
            .blocking(move |context| {
                context
                    .backend()
                    .database()
                    .provider_secret_cleanup_done(&record)
                    .map_err(model_error)
            })
            .await?;
    }
    Ok(())
}

pub async fn provider_account_save(
    context: &ApiContext,
    request: dto::ProviderAccountSaveRequest,
) -> Result<dto::ProviderAccountView, ApiError> {
    let _guard = context.provider_write_guard().await;
    if context.shutdown_token().is_cancelled() {
        return Err(api_error(
            ApiErrorCode::Cancelled,
            "application is shutting down",
        ));
    }
    let mut keyless = serde_json::to_value(&request).map_err(|_| api_error(ApiErrorCode::Internal, "provider request cannot be encoded"))?;
    keyless["api_key"] = serde_json::json!(request.api_key.as_ref().is_some_and(|key| !key.trim().is_empty()));
    let digest = digest(&keyless, &request.client_operation_id)?;
    if let Some(view) = replay::<dto::ProviderAccountView>(
        context,
        "provider_account_save",
        request.client_operation_id.clone(),
        digest.clone(),
    )
    .await?
    {
        if let Some(key) = request.api_key.as_ref().filter(|key| !key.trim().is_empty()) {
            match super::providers::account(context, view.id.clone()).await {
                Ok(account) => {
                    let reference = SecretRef::from_uuid(identity("key", &request.client_operation_id, &digest));
                    match context.secret_store().load(&reference, &SecretPurpose::ProviderApiKey { owner: account.secret_owner_id }).await {
                        Ok(stored) if !stored.with(|stored| stored == key.trim()) => {
                            return Err(api_error(ApiErrorCode::Conflict, "operation id was already used with another credential"));
                        }
                        Ok(_) | Err(lettuce_settings::SecretStoreError::Missing) => {}
                        Err(_) => return Err(api_error(ApiErrorCode::Unavailable, "replay credential cannot be read")),
                    }
                }
                Err(error) if error.code == ApiErrorCode::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        cleanup_secrets(context, false).await?;
        return Ok(view);
    }
    if request.account.label.trim().is_empty() {
        return Err(invalid_field("account.label", "provider label is empty"));
    }
    let descriptor = lettuce_providers::provider_descriptor(&request.account.provider_kind)
        .ok_or_else(|| invalid_field("account.provider_kind", "unknown provider kind"))?;
    let config: ProviderConfig = serde_json::from_value(request.account.config)
        .map_err(|_| invalid_field("account.config", "invalid provider configuration"))?;
    let old = match &request.account.id {
        Some(id) => Some(super::providers::account(context, id.clone()).await?),
        None => None,
    };
    let expected = request
        .expected_revision
        .map(|value| super::lorebooks::revision(value, "expected_revision"))
        .transpose()?;
    if old.as_ref().map(|account| account.revision) != expected {
        return Err(api_error(
            ApiErrorCode::Conflict,
            "provider account revision is stale",
        ));
    }
    if let ProviderConfig::Ollama(config) = &config {
        if config
            .sprout
            .as_ref()
            .is_some_and(|sprout| sprout.api_key_ref.is_some())
        {
            return Err(invalid_field(
                "account.config",
                "secret references cannot be supplied",
            ));
        }
    }
    let key = request.api_key.filter(|key| !key.trim().is_empty());
    if key.is_some() && request.clear_api_key {
        return Err(invalid_field(
            "api_key",
            "cannot replace and clear the same key",
        ));
    }
    let owner = old.as_ref().map_or_else(
        || SecretOwnerId::from_uuid(identity("owner", &request.client_operation_id, &digest)),
        |account| account.secret_owner_id,
    );
    let reference = key
        .as_ref()
        .map(|_| SecretRef::from_uuid(identity("key", &request.client_operation_id, &digest)));
    let mut account = ProviderAccount {
        id: old.as_ref().map_or_else(
            || {
                ProviderAccountId::from_uuid(identity(
                    "account",
                    &request.client_operation_id,
                    &digest,
                ))
            },
            |account| account.id,
        ),
        secret_owner_id: owner,
        provider_kind: descriptor.kind.into(),
        protocol: descriptor.protocol,
        label: request.account.label,
        endpoint: request.account.base_url,
        enabled: request.account.enabled,
        streaming_enabled: request.account.streaming_enabled,
        allow_invalid_tls: request.account.allow_invalid_tls,
        api_key_ref: if request.clear_api_key {
            None
        } else {
            reference.or_else(|| old.as_ref().and_then(|account| account.api_key_ref))
        },
        secret_headers: old
            .as_ref()
            .map_or_else(Vec::new, |account| account.secret_headers.clone()),
        config,
        revision: Revision::INITIAL,
        created_at: context.now(),
        updated_at: context.now(),
    };
    if let (ProviderConfig::Ollama(config), Some(old)) = (&mut account.config, &old) {
        if let (Some(sprout), ProviderConfig::Ollama(previous)) = (&mut config.sprout, &old.config)
        {
            sprout.api_key_ref = previous
                .sprout
                .as_ref()
                .and_then(|sprout| sprout.api_key_ref);
        }
    }
    lettuce_models::validate_provider_connection(&account)
        .map_err(|_| invalid_field("account", "invalid provider connection"))?;
    if let (Some(reference), Some(key)) = (reference, key) {
        let value = SecretValue::new(key.trim())
            .map_err(|_| invalid_field("api_key", "invalid API key"))?;
        let record = SecretRecord::new(reference, SecretPurpose::ProviderApiKey { owner });
        let staged = record.clone();
        context
            .blocking(move |context| {
                context
                    .backend()
                    .database()
                    .stage_provider_secret(&staged)
                    .map_err(model_error)
            })
            .await?;
        context
            .secret_store()
            .put(record, value, None)
            .await
            .map_err(|_| {
                api_error(
                    ApiErrorCode::Unavailable,
                    "provider credential cannot be stored",
                )
            })?;
    }
    if context.shutdown_token().is_cancelled() {
        return Err(api_error(
            ApiErrorCode::Cancelled,
            "application is shutting down",
        ));
    }
    let key_set = super::providers::api_key_set(context, &account).await?;
    let key = request.client_operation_id;
    let view = context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "provider_account_save",
                    &key,
                    &digest,
                    context.now(),
                    |scope| {
                        let account = scope
                            .save_provider_account(account, expected)
                            .map_err(|error| Failure(model_error(error)))?;
                        super::providers::account_view(account, key_set).map_err(Failure)
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await?;
    cleanup_secrets(context, false).await?;
    Ok(view)
}

pub async fn provider_account_delete(
    context: &ApiContext,
    request: dto::ProviderAccountDeleteRequest,
) -> Result<(), ApiError> {
    let _guard = context.provider_write_guard().await;
    if context.shutdown_token().is_cancelled() {
        return Err(api_error(
            ApiErrorCode::Cancelled,
            "application is shutting down",
        ));
    }
    let digest = digest(&request, &request.client_operation_id)?;
    let id: ProviderAccountId = parse_id(&request.account_id, "account_id")?;
    let expected = super::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let (characters, groups) = context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .commit_api_operation(
                    "provider_account_delete",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .delete_provider_account(
                                id,
                                expected,
                                request.delete_models,
                                context.now(),
                            )
                            .map_err(|error| Failure(model_error(error)))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
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
    cleanup_secrets(context, false).await?;
    Ok(())
}

fn refresh_clients(context: &ApiContext) -> Result<(), ApiError> {
    let backend = context.backend();
    let mut json = backend
        .provider_json_clients
        .lock()
        .map_err(|_| api_error(ApiErrorCode::Internal, "provider clients are unavailable"))?;
    let mut bulk = backend
        .provider_bulk_clients
        .lock()
        .map_err(|_| api_error(ApiErrorCode::Internal, "provider clients are unavailable"))?;
    let policy = backend
        .tls_policy()
        .map_err(|_| api_error(ApiErrorCode::Unavailable, "TLS settings cannot be read"))?;
    json.retain(|client| client.upgrade().is_some());
    bulk.retain(|client| client.upgrade().is_some());
    for client in json
        .iter()
        .filter_map(lettuce_network::WeakJsonClient::upgrade)
    {
        client.reload_tls(&policy).map_err(|_| {
            api_error(
                ApiErrorCode::Unavailable,
                "provider TLS client cannot be rebuilt",
            )
        })?;
    }
    for client in bulk
        .iter()
        .filter_map(lettuce_network::WeakBulkHttpClient::upgrade)
    {
        client.reload_tls(&policy).map_err(|_| {
            api_error(
                ApiErrorCode::Unavailable,
                "provider TLS client cannot be rebuilt",
            )
        })?;
    }
    Ok(())
}
pub async fn certificates_import(
    context: &ApiContext,
    request: dto::CertificatesImportRequest,
) -> Result<dto::CertificatesView, ApiError> {
    let key = request.client_operation_id.clone();
    let digest = digest(&request, &key)?;
    if let Some(view) = replay(context, "certificates_import", key.clone(), digest.clone()).await? {
        context.blocking(refresh_clients).await?;
        return Ok(view);
    }
    let certificate_digest = digest.clone();
    let certificate = context
        .blocking(move |context| {
            use std::io::Read;
            let description = context
                .files()
                .describe(&request.source.uri)
                .map_err(IntoApiError::into_api_error)?;
            let cap = lettuce_settings::MAX_CERTIFICATE_PEM_BYTES;
            if description.size > cap as u64 {
                return Err(invalid_field("source", "certificate exceeds the PEM size limit"));
            }
            let mut pem = String::new();
            context
                .files()
                .open(&request.source.uri)
                .map_err(IntoApiError::into_api_error)?
                .take(cap as u64 + 1)
                .read_to_string(&mut pem)
                .map_err(|_| invalid_field("source", "certificate is not UTF-8 PEM"))?;
            if pem.len() > cap {
                return Err(invalid_field("source", "certificate exceeds the PEM size limit"));
            }
            lettuce_network::validate_tls_policy(&lettuce_network::TlsPolicy {
                trusted_roots_pem: vec![pem.clone()],
            })
            .map_err(|_| invalid_field("source", "invalid PEM certificate"))?;
            Ok(
                TrustedCertificate {
                    id: identity("certificate", &request.client_operation_id, &certificate_digest),
                    name: description.name,
                    pem,
                    imported_at: context.now().get(),
                }
            )
        })
        .await?;
    let result = context
        .blocking(move |context| {
            let view = context
                .backend()
                .database()
                .commit_api_operation(
                    "certificates_import",
                    &key,
                    &digest,
                    context.now(),
                    |scope| {
                        let (certificates, revision) = scope
                            .import_certificate(certificate)
                            .map_err(|error| Failure(match error {
                                lettuce_database::CertificateImportError::Model(error) => model_error(error),
                                lettuce_database::CertificateImportError::AlreadyImported { certificate_id } => ApiError {
                                    code: ApiErrorCode::Conflict,
                                    message: "certificate is already imported".into(),
                                    details: Some(dto::ApiErrorDetails::CertificateAlreadyImported { certificate_id: certificate_id.to_string() }),
                                },
                            }))?;
                        Ok::<_, Failure>(super::providers::certificate_view(certificates, revision))
                    },
                )
                .map_err(|failure: Failure| failure.0)?;
            refresh_clients(context)?;
            Ok(view)
        })
        .await?;
    context.emit(dto::ApiEvent::SettingsChanged {
        section: "certificates".into(),
    });
    Ok(result)
}
pub async fn certificates_remove(
    context: &ApiContext,
    request: dto::CertificatesRemoveRequest,
) -> Result<dto::CertificatesView, ApiError> {
    let id = parse_id::<uuid::Uuid>(&request.certificate_id, "certificate_id")?;
    let expected = super::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let view = context
        .blocking(move |context| {
            let (certificates, revision) = context
                .backend()
                .database()
                .remove_certificate_cas(id, expected)
                .map_err(model_error)?;
            refresh_clients(context)?;
            Ok(super::providers::certificate_view(certificates, revision))
        })
        .await?;
    context.emit(dto::ApiEvent::SettingsChanged {
        section: "certificates".into(),
    });
    Ok(view)
}
