use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use lettuce_models::{PromptCacheRetention, PromptCaching, ResolvedChatProfile};
use lettuce_network::{JsonClient, MAX_REQUEST_BYTES};
use lettuce_settings::{SecretPurpose, SecretStatus, SecretStore};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;

use crate::{
    common::{AdapterError, Credentials, generation_policy, load_auth, load_secret_headers},
    gemini_generate::{Content, GeminiWireProvider, GenerateRequest},
};

type CacheKey = [u8; 32];

#[derive(Debug)]
struct CachedContentEntry {
    name: String,
    expires_at: Instant,
}

/// Process-local index of Gemini resources. The resources themselves live at
/// Google and expire naturally; persisting these names would make restarts
/// depend on remote state we do not own.
#[derive(Debug, Default)]
pub(crate) struct GeminiCache {
    entries: Mutex<HashMap<CacheKey, CachedContentEntry>>,
    flights: Mutex<HashMap<CacheKey, Weak<AsyncMutex<()>>>>,
}

#[derive(Debug)]
pub(crate) struct PreparedCache {
    pub(crate) key: CacheKey,
    pub(crate) name: String,
}

impl GeminiCache {
    pub(crate) async fn prepare<S: SecretStore + ?Sized>(
        &self,
        provider: &dyn GeminiWireProvider,
        secret_store: &S,
        network: &JsonClient,
        profile: &ResolvedChatProfile,
        base: &str,
        request: &GenerateRequest,
    ) -> Result<Option<PreparedCache>, AdapterError> {
        let Some(PromptCaching::Enabled { retention }) = profile.parameters.prompt_caching else {
            return Ok(None);
        };
        let (ttl, lifetime) = retention_policy(retention)?;
        let Some((prefix, _)) = request.cache_partition() else {
            return Ok(None);
        };
        let credentials = Credentials::from(profile);
        let credential_statuses = credential_statuses(secret_store, &credentials).await?;
        let key = cache_key(profile, base, request, prefix, ttl, &credential_statuses)?;
        if let Some(name) = self.lookup(key) {
            return Ok(Some(PreparedCache { key, name }));
        }
        let flight = self.flight(key);
        let _guard = flight.lock().await;
        if let Some(name) = self.lookup(key) {
            return Ok(Some(PreparedCache { key, name }));
        }

        let create = CachedContentRequest {
            model: format!("models/{}", profile.external_model_id),
            contents: prefix,
            system_instruction: request.system_instruction.as_ref(),
            ttl,
        };
        let body = serde_json::to_vec(&create).map_err(|_| AdapterError::Rejected)?;
        if body.len() > MAX_REQUEST_BYTES {
            tracing::warn!(
                provider = "gemini",
                cache_result = "request_too_large",
                "explicit prompt cache unavailable; sending the full request"
            );
            return Ok(None);
        }

        let auth = load_auth(
            provider.auth(&profile.provider_config)?,
            secret_store,
            &credentials,
        )
        .await?;
        let secret_headers = load_secret_headers(secret_store, &credentials).await?;
        let response = match network
            .post_json(
                base,
                "/cachedContents",
                body,
                provider.static_headers(),
                auth,
                secret_headers,
                generation_policy(&credentials),
            )
            .await
        {
            Ok(response) => response,
            Err(_) => {
                tracing::warn!(
                    provider = "gemini",
                    cache_result = "transport_error",
                    "explicit prompt cache unavailable; sending the full request"
                );
                return Ok(None);
            }
        };
        if !(200..300).contains(&response.status) {
            tracing::warn!(
                provider = "gemini",
                cache_result = "provider_rejected",
                status = response.status,
                "explicit prompt cache unavailable; sending the full request"
            );
            return Ok(None);
        }
        let Ok(created) = serde_json::from_slice::<CachedContentResponse>(&response.body) else {
            tracing::warn!(
                provider = "gemini",
                cache_result = "malformed_response",
                "explicit prompt cache unavailable; sending the full request"
            );
            return Ok(None);
        };
        if !valid_cache_name(&created.name) {
            tracing::warn!(
                provider = "gemini",
                cache_result = "missing_name",
                "explicit prompt cache unavailable; sending the full request"
            );
            return Ok(None);
        }

        self.insert(key, created.name.clone(), lifetime);
        Ok(Some(PreparedCache {
            key,
            name: created.name,
        }))
    }

    pub(crate) fn evict(&self, key: CacheKey) {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&key);
    }

    fn lookup(&self, key: CacheKey) -> Option<String> {
        let now = Instant::now();
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        entries.retain(|_, entry| entry.expires_at > now);
        entries.get(&key).map(|entry| entry.name.clone())
    }

    fn insert(&self, key: CacheKey, name: String, lifetime: Duration) {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                key,
                CachedContentEntry {
                    name,
                    expires_at: Instant::now() + lifetime,
                },
            );
    }

    fn flight(&self, key: CacheKey) -> Arc<AsyncMutex<()>> {
        let mut flights = self
            .flights
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        flights.retain(|_, flight| flight.strong_count() > 0);
        if let Some(flight) = flights.get(&key).and_then(Weak::upgrade) {
            return flight;
        }
        let flight = Arc::new(AsyncMutex::new(()));
        flights.insert(key, Arc::downgrade(&flight));
        flight
    }
}

async fn credential_statuses<S: SecretStore + ?Sized>(
    secret_store: &S,
    credentials: &Credentials<'_>,
) -> Result<Vec<SecretStatus>, AdapterError> {
    let api_key_ref = credentials
        .api_key_ref
        .ok_or(AdapterError::CredentialRejected)?;
    let mut statuses = Vec::with_capacity(credentials.secret_headers.len() + 1);
    statuses.push(
        secret_store
            .status(
                &api_key_ref,
                &SecretPurpose::ProviderApiKey {
                    owner: credentials.owner,
                },
            )
            .await
            .map_err(AdapterError::from_secret)?,
    );
    for header in credentials.secret_headers {
        statuses.push(
            secret_store
                .status(
                    &header.secret_ref,
                    &SecretPurpose::ProviderSecretHeader {
                        owner: credentials.owner,
                        name: header.name.clone(),
                    },
                )
                .await
                .map_err(AdapterError::from_secret)?,
        );
    }
    Ok(statuses)
}

fn retention_policy(
    retention: PromptCacheRetention,
) -> Result<(&'static str, Duration), AdapterError> {
    match retention {
        PromptCacheRetention::FiveMinutes => Ok(("300s", Duration::from_secs(300))),
        PromptCacheRetention::OneHour => Ok(("3600s", Duration::from_secs(3_600))),
        PromptCacheRetention::InMemory | PromptCacheRetention::TwentyFourHours => {
            Err(AdapterError::Rejected)
        }
    }
}

fn cache_key(
    profile: &ResolvedChatProfile,
    base: &str,
    request: &GenerateRequest,
    prefix: &[Content],
    ttl: &str,
    credential_statuses: &[SecretStatus],
) -> Result<CacheKey, AdapterError> {
    #[derive(Serialize)]
    struct KeyPayload<'a> {
        provider_account_id: lettuce_types::ProviderAccountId,
        provider_account_revision: lettuce_types::Revision,
        endpoint: &'a str,
        model: &'a str,
        system_instruction: Option<&'a Content>,
        contents: &'a [Content],
        ttl: &'a str,
        credential_statuses: &'a [SecretStatus],
    }

    let payload = serde_json::to_vec(&KeyPayload {
        provider_account_id: profile.provider_account_id,
        provider_account_revision: profile.provider_account_revision,
        endpoint: base,
        model: &profile.external_model_id,
        system_instruction: request.system_instruction.as_ref(),
        contents: prefix,
        ttl,
        credential_statuses,
    })
    .map_err(|_| AdapterError::Rejected)?;
    Ok(*blake3::hash(&payload).as_bytes())
}

fn valid_cache_name(name: &str) -> bool {
    name == name.trim()
        && name.len() <= 1_024
        && name
            .strip_prefix("cachedContents/")
            .is_some_and(|id| !id.is_empty() && !id.chars().any(char::is_control))
}

#[derive(Serialize)]
struct CachedContentRequest<'a> {
    model: String,
    contents: &'a [Content],
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<&'a Content>,
    ttl: &'static str,
}

#[derive(Deserialize)]
struct CachedContentResponse {
    name: String,
}
