use std::borrow::Cow;

use lettuce_models::{ProviderAccount, ProviderConfig, ProviderProtocol};
use lettuce_network::{JsonClient, JsonResponse};
use lettuce_settings::{HeaderName, SecretStore};

use crate::anthropic_messages::ANTHROPIC_HEADERS;
use crate::common::{
    ACCEPT_ONLY, AdapterError, AuthPlan, Credentials, custom_config, load_auth,
    load_secret_headers, probe_policy,
};
use crate::descriptor::KeyVerification;

enum Probe {
    Get { path: Cow<'static, str> },
    PostNull { path: Cow<'static, str> },
    AlwaysValid,
}

fn versioned(endpoint: &str, versioned: &'static str, bare: &'static str) -> &'static str {
    let trimmed = endpoint.trim_end_matches('/');
    if trimmed.ends_with("/v1") || trimmed.ends_with("/v1beta") {
        bare
    } else {
        versioned
    }
}

/// Mirrors the legacy `verify_provider_api_key` probes: which URL is hit,
/// with which credential header, and how the status is judged.
pub(crate) async fn verify_api_key<S: SecretStore + ?Sized>(
    secret_store: &S,
    network: &JsonClient,
    account: &ProviderAccount,
) -> Result<KeyVerification, AdapterError> {
    let kind = account.provider_kind.to_ascii_lowercase();
    let credentials = Credentials::from(account);
    let descriptor = crate::catalog::provider_descriptor(&kind).ok_or(AdapterError::Rejected)?;
    let endpoint = account
        .endpoint
        .as_deref()
        .or(descriptor.default_endpoint)
        .ok_or(AdapterError::Rejected)?;
    let (probe, auth_plan, headers): (
        Probe,
        AuthPlan,
        &'static [lettuce_network::JsonStaticHeader],
    ) = match (descriptor.protocol, kind.as_str()) {
        (_, "chutes" | "chutes.ai") => (Probe::AlwaysValid, AuthPlan::None, &ACCEPT_ONLY),
        (_, "openrouter") => (
            Probe::Get {
                path: Cow::Borrowed("/v1/key"),
            },
            AuthPlan::Bearer,
            &ACCEPT_ONLY,
        ),
        (_, "groq") => (
            Probe::Get {
                path: Cow::Borrowed(if endpoint.trim_end_matches('/').ends_with("/openai") {
                    "/v1/models"
                } else {
                    "/openai/v1/models"
                }),
            },
            AuthPlan::Bearer,
            &ACCEPT_ONLY,
        ),
        (ProviderProtocol::Gemini, _) => (
            Probe::Get {
                path: Cow::Borrowed("/models"),
            },
            AuthPlan::Header(
                HeaderName::new("x-goog-api-key").map_err(|_| AdapterError::Rejected)?,
            ),
            &ACCEPT_ONLY,
        ),
        (_, "zai" | "z.ai") => (
            Probe::PostNull {
                path: Cow::Borrowed("/chat/completions"),
            },
            AuthPlan::Bearer,
            &ACCEPT_ONLY,
        ),
        (_, "mistral") => (
            Probe::Get {
                path: Cow::Borrowed(versioned(endpoint, "/v1/models", "/models")),
            },
            AuthPlan::Header(HeaderName::new("x-api-key").map_err(|_| AdapterError::Rejected)?),
            &ACCEPT_ONLY,
        ),
        (ProviderProtocol::Anthropic, "anthropic") => (
            Probe::Get {
                path: Cow::Borrowed(versioned(endpoint, "/v1/models", "/models")),
            },
            AuthPlan::Header(HeaderName::new("x-api-key").map_err(|_| AdapterError::Rejected)?),
            &ANTHROPIC_HEADERS,
        ),
        (_, "custom" | "custom-anthropic") => {
            let config = custom_config(&account.config)?;
            (
                Probe::PostNull {
                    path: Cow::Owned(config.chat_path.clone()),
                },
                crate::common::custom_auth_plan(&config.auth),
                if descriptor.protocol == ProviderProtocol::Anthropic {
                    &ANTHROPIC_HEADERS
                } else {
                    &ACCEPT_ONLY
                },
            )
        }
        (ProviderProtocol::OpenAiCompatible, _) => (
            Probe::Get {
                path: Cow::Borrowed(versioned(endpoint, "/v1/models", "/models")),
            },
            AuthPlan::Bearer,
            &ACCEPT_ONLY,
        ),
        (ProviderProtocol::Ollama, _) => (
            Probe::Get {
                path: Cow::Borrowed("/api/tags"),
            },
            AuthPlan::OptionalBearer,
            &ACCEPT_ONLY,
        ),
        _ => return Err(AdapterError::Rejected),
    };
    if matches!(probe, Probe::AlwaysValid) {
        return Ok(KeyVerification {
            valid: true,
            status: None,
        });
    }
    if credentials.api_key_ref.is_none() {
        return Ok(KeyVerification {
            valid: false,
            status: None,
        });
    }
    if !matches!(
        account.config,
        ProviderConfig::Standard | ProviderConfig::Custom(_)
    ) {
        return Err(AdapterError::Rejected);
    }
    let auth = load_auth(auth_plan, secret_store, &credentials).await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    let policy = probe_policy(&credentials);
    let (response, post) = match probe {
        Probe::Get { path } => (
            network
                .get_json(endpoint, &path, headers, auth, secret_headers, policy)
                .await?,
            false,
        ),
        Probe::PostNull { path } => (
            network
                .post_json(
                    endpoint,
                    &path,
                    b"null".to_vec(),
                    headers,
                    auth,
                    secret_headers,
                    policy,
                )
                .await?,
            true,
        ),
        Probe::AlwaysValid => unreachable!("handled above"),
    };
    Ok(judge(&response, post))
}

fn judge(response: &JsonResponse, post: bool) -> KeyVerification {
    let status = response.status;
    let valid = if post {
        !matches!(status, 401 | 403)
    } else {
        match status {
            200 => true,
            401 | 403 => false,
            _ => serde_json::from_slice::<serde_json::Value>(&response.body)
                .ok()
                .and_then(|json| json.get("data")?.as_array().map(|items| !items.is_empty()))
                .unwrap_or(false),
        }
    };
    KeyVerification {
        valid,
        status: Some(status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u16, body: &str) -> JsonResponse {
        JsonResponse {
            status,
            body: body.as_bytes().to_vec(),
            request_id: None,
            retry_after: None,
        }
    }

    #[test]
    fn judges_like_legacy() {
        assert!(judge(&response(200, "{}"), false).valid);
        assert!(!judge(&response(401, "{}"), false).valid);
        assert!(!judge(&response(403, "{}"), false).valid);
        assert!(judge(&response(404, r#"{"data":[{"id":"m"}]}"#), false).valid);
        assert!(!judge(&response(500, r#"{"data":[]}"#), false).valid);
        assert!(judge(&response(400, "{}"), true).valid);
        assert!(!judge(&response(401, "{}"), true).valid);
    }
}
