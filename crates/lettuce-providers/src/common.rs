use lettuce_conversations::{InferenceRequest, PortError, ProviderFailure, ProviderFailureKind};
use lettuce_models::{
    PromptCaching, ProviderAccount, ReasoningMode, ResolvedChatParameters, ResolvedChatProfile,
    SecretHeader,
};
use lettuce_network::{
    JsonAuth, JsonClientError, JsonSecretHeader, JsonStaticHeader, RequestPolicy, RequestTimeout,
};
use lettuce_settings::{
    HeaderName, SecretOwnerId, SecretPurpose, SecretRef, SecretStore, SecretStoreError, SecretValue,
};

pub(crate) const FALLBACK_MAX_OUTPUT_TOKENS: u32 = 4096;
pub(crate) const ACCEPT_ONLY: [JsonStaticHeader; 1] = [JsonStaticHeader {
    name: "accept",
    value: "application/json",
}];
pub(crate) const NO_HEADERS: [JsonStaticHeader; 0] = [];
pub(crate) const STANDARD_HEADERS: [JsonStaticHeader; 2] = [
    JsonStaticHeader {
        name: "accept",
        value: "application/json",
    },
    JsonStaticHeader {
        name: "user-agent",
        value: concat!("LettuceAI/", env!("CARGO_PKG_VERSION")),
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AdapterError {
    Rejected,
    CredentialRejected,
    SecretUnavailable,
    Provider(ProviderFailure),
    MalformedResponse,
    EmptyResponse,
    Transport,
    Cancelled,
}

impl AdapterError {
    pub(crate) fn from_secret(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::Missing | SecretStoreError::PurposeMismatch => {
                Self::CredentialRejected
            }
            SecretStoreError::Unavailable(_)
            | SecretStoreError::Backend(_)
            | SecretStoreError::StaleGeneration
            | SecretStoreError::GenerationOverflow => Self::SecretUnavailable,
        }
    }

    pub(crate) fn from_response(response: &lettuce_network::JsonResponse) -> Option<Self> {
        if (200..300).contains(&response.status) {
            return None;
        }
        let (code, message) = provider_error_details(&response.body);
        let kind = match response.status {
            401 | 403 => ProviderFailureKind::CredentialRejected,
            408 | 429 | 500..=599 => ProviderFailureKind::Unavailable,
            _ => ProviderFailureKind::RequestRejected,
        };
        Some(Self::Provider(ProviderFailure {
            kind,
            status: response.status,
            code,
            message,
            request_id: response.request_id.clone(),
        }))
    }
}

fn provider_error_details(body: &[u8]) -> (Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return (None, None);
    };
    let error = value.get("error").unwrap_or(&value);
    let message = error
        .as_str()
        .or_else(|| error.get("message").and_then(serde_json::Value::as_str))
        .or_else(|| value.get("message").and_then(serde_json::Value::as_str))
        .and_then(|value| bounded_diagnostic(value, 2_048));
    let code = error
        .get("code")
        .or_else(|| error.get("type"))
        .or_else(|| error.get("status"))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| value.as_i64().map(|number| number.to_string()))
        })
        .and_then(|value| bounded_diagnostic(&value, 128));
    (code, message)
}

fn bounded_diagnostic(value: &str, max_bytes: usize) -> Option<String> {
    let clean: String = value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        return None;
    }
    let boundary = clean
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= max_bytes)
        .last()
        .unwrap_or(0);
    let end = if clean.len() <= max_bytes {
        clean.len()
    } else {
        boundary
    };
    Some(clean[..end].to_owned())
}

impl From<JsonClientError> for AdapterError {
    fn from(error: JsonClientError) -> Self {
        match error {
            JsonClientError::InvalidUrl | JsonClientError::InvalidRequest => Self::Rejected,
            JsonClientError::RequestTooLarge => Self::Rejected,
            JsonClientError::ResponseTooLarge
            | JsonClientError::Transport
            | JsonClientError::ClientConfiguration => Self::Transport,
        }
    }
}

impl From<AdapterError> for PortError {
    fn from(error: AdapterError) -> Self {
        match error {
            AdapterError::MalformedResponse | AdapterError::EmptyResponse => PortError::Empty,
            AdapterError::Rejected | AdapterError::CredentialRejected => PortError::Rejected,
            AdapterError::SecretUnavailable | AdapterError::Transport => PortError::Unavailable,
            AdapterError::Cancelled => PortError::Cancelled,
            AdapterError::Provider(failure) => PortError::Provider(failure),
        }
    }
}

/// The secret references and transport opt-ins of one provider account, taken
/// from either a resolved chat profile or the stored account.
pub(crate) struct Credentials<'a> {
    pub(crate) owner: SecretOwnerId,
    pub(crate) api_key_ref: Option<SecretRef>,
    pub(crate) secret_headers: &'a [SecretHeader],
    pub(crate) allow_invalid_tls: bool,
}

/// Legacy honoured `allowInvalidTls` only for local and custom providers
/// (`old-code/src-tauri/src/tls.rs`); hosted providers never skip validation.
pub(crate) fn tls_opt_in_allowed(kind: &str) -> bool {
    crate::catalog::provider_descriptor(kind).is_some_and(|descriptor| descriptor.endpoint_editable)
}

impl<'a> From<&'a ResolvedChatProfile> for Credentials<'a> {
    fn from(profile: &'a ResolvedChatProfile) -> Self {
        Self {
            owner: profile.secret_owner_id,
            api_key_ref: profile.api_key_ref,
            secret_headers: &profile.secret_headers,
            allow_invalid_tls: profile.allow_invalid_tls
                && tls_opt_in_allowed(&profile.provider_kind),
        }
    }
}

impl<'a> From<&'a ProviderAccount> for Credentials<'a> {
    fn from(account: &'a ProviderAccount) -> Self {
        Self {
            owner: account.secret_owner_id,
            api_key_ref: account.api_key_ref,
            secret_headers: &account.secret_headers,
            allow_invalid_tls: account.allow_invalid_tls
                && tls_opt_in_allowed(&account.provider_kind),
        }
    }
}

pub(crate) enum AuthPlan {
    Bearer,
    OptionalBearer,
    Header(HeaderName),
    Query(String),
    None,
}

pub(crate) fn validate_common_request(request: &InferenceRequest) -> Result<(), AdapterError> {
    let profile = &request.profile;
    if profile.tool_policy != lettuce_conversations::ToolPolicy::Disabled
        || profile.output_policy != lettuce_conversations::OutputPolicy::Plain
        || !request.media_grants.is_empty()
    {
        return Err(AdapterError::Rejected);
    }
    request
        .context
        .validate()
        .map_err(|_| AdapterError::Rejected)
}

pub(crate) fn reject_unsupported_features(
    parameters: &ResolvedChatParameters,
) -> Result<(), AdapterError> {
    if parameters.reasoning_mode == Some(ReasoningMode::Enabled)
        || parameters.reasoning_effort.is_some()
        || parameters.reasoning_budget_tokens.is_some()
        || parameters.total_completion_allowance != parameters.visible_max_output_tokens
    {
        return Err(AdapterError::Rejected);
    }
    Ok(())
}

pub(crate) fn validate_supported_reasoning(
    parameters: &ResolvedChatParameters,
) -> Result<(), AdapterError> {
    if parameters.reasoning_mode != Some(ReasoningMode::Enabled)
        && (parameters.reasoning_effort.is_some()
            || parameters.reasoning_budget_tokens.is_some()
            || parameters.total_completion_allowance != parameters.visible_max_output_tokens)
    {
        return Err(AdapterError::Rejected);
    }
    if parameters.reasoning_mode == Some(ReasoningMode::Enabled) {
        let expected = match (
            parameters.visible_max_output_tokens,
            parameters.reasoning_budget_tokens,
        ) {
            (Some(visible), Some(budget)) => visible.checked_add(budget),
            (Some(visible), None) => Some(visible),
            (None, _) => None,
        };
        if parameters.total_completion_allowance != expected {
            return Err(AdapterError::Rejected);
        }
    }
    Ok(())
}

pub(crate) fn validate_prompt_caching(
    support: crate::descriptor::PromptCachingSupport,
    parameters: &ResolvedChatParameters,
) -> Result<(), AdapterError> {
    let Some(PromptCaching::Enabled { retention }) = parameters.prompt_caching else {
        return Ok(());
    };
    if support.retentions().contains(&retention) {
        Ok(())
    } else {
        Err(AdapterError::Rejected)
    }
}

pub(crate) fn max_output_tokens(parameters: &ResolvedChatParameters) -> u32 {
    parameters
        .visible_max_output_tokens
        .unwrap_or(FALLBACK_MAX_OUTPUT_TOKENS)
}

pub(crate) async fn load_auth<S: SecretStore + ?Sized>(
    plan: AuthPlan,
    secret_store: &S,
    credentials: &Credentials<'_>,
) -> Result<JsonAuth, AdapterError> {
    Ok(match plan {
        AuthPlan::Bearer => JsonAuth::Bearer(load_api_key(secret_store, credentials).await?),
        AuthPlan::OptionalBearer => match credentials.api_key_ref {
            Some(_) => JsonAuth::Bearer(load_api_key(secret_store, credentials).await?),
            None => JsonAuth::None,
        },
        AuthPlan::Header(name) => JsonAuth::Header {
            name,
            value: load_api_key(secret_store, credentials).await?,
        },
        AuthPlan::Query(name) => JsonAuth::Query {
            name,
            value: load_api_key(secret_store, credentials).await?,
        },
        AuthPlan::None => JsonAuth::None,
    })
}

pub(crate) async fn load_api_key<S: SecretStore + ?Sized>(
    secret_store: &S,
    credentials: &Credentials<'_>,
) -> Result<SecretValue, AdapterError> {
    let reference = credentials
        .api_key_ref
        .ok_or(AdapterError::CredentialRejected)?;
    secret_store
        .load(
            &reference,
            &SecretPurpose::ProviderApiKey {
                owner: credentials.owner,
            },
        )
        .await
        .map_err(AdapterError::from_secret)
}

pub(crate) async fn load_secret_headers<S: SecretStore + ?Sized>(
    secret_store: &S,
    credentials: &Credentials<'_>,
) -> Result<Vec<JsonSecretHeader>, AdapterError> {
    let mut headers = Vec::with_capacity(credentials.secret_headers.len());
    for header in credentials.secret_headers {
        let value = secret_store
            .load(
                &header.secret_ref,
                &SecretPurpose::ProviderSecretHeader {
                    owner: credentials.owner,
                    name: header.name.clone(),
                },
            )
            .await
            .map_err(AdapterError::from_secret)?;
        headers.push(JsonSecretHeader {
            name: header.name.clone(),
            value,
        });
    }
    Ok(headers)
}

pub(crate) fn custom_auth_plan(auth: &lettuce_models::CustomAuth) -> AuthPlan {
    match auth {
        lettuce_models::CustomAuth::Bearer => AuthPlan::Bearer,
        lettuce_models::CustomAuth::Header { name } => AuthPlan::Header(name.clone()),
        lettuce_models::CustomAuth::Query { name } => AuthPlan::Query(name.as_str().to_owned()),
        lettuce_models::CustomAuth::None => AuthPlan::None,
    }
}

pub(crate) fn generation_policy(credentials: &Credentials<'_>) -> RequestPolicy {
    RequestPolicy {
        timeout: RequestTimeout::Generation,
        allow_invalid_tls: credentials.allow_invalid_tls,
    }
}

pub(crate) fn probe_policy(credentials: &Credentials<'_>) -> RequestPolicy {
    RequestPolicy {
        timeout: RequestTimeout::Probe,
        allow_invalid_tls: credentials.allow_invalid_tls,
    }
}

pub(crate) fn custom_config(
    config: &lettuce_models::ProviderConfig,
) -> Result<&lettuce_models::CustomProviderConfig, AdapterError> {
    match config {
        lettuce_models::ProviderConfig::Custom(config) => Ok(config),
        lettuce_models::ProviderConfig::Standard => Err(AdapterError::Rejected),
    }
}

/// The legacy OpenAI `data[]` model list shape, also used as the fallback
/// parser for custom accounts whose configured paths match nothing.
pub(crate) fn parse_openai_model_list(payload: &serde_json::Value) -> Vec<RemoteModel> {
    let Some(items) = payload.get("data").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?;
            let modalities = |key: &str| {
                item.get("architecture")
                    .and_then(|architecture| architecture.get(key))
                    .or_else(|| item.get(key))
                    .and_then(serde_json::Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
            };
            Some(RemoteModel {
                id: id.to_owned(),
                display_name: item
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                description: item
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                context_length: item
                    .get("context_length")
                    .and_then(serde_json::Value::as_u64),
                input_modalities: modalities("input_modalities"),
                output_modalities: modalities("output_modalities"),
                supported_endpoints: item
                    .get("supported_endpoints")
                    .and_then(serde_json::Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    }),
                input_price: item
                    .get("pricing")
                    .and_then(|pricing| pricing.get("prompt"))
                    .and_then(value_to_f64),
                output_price: item
                    .get("pricing")
                    .and_then(|pricing| pricing.get("completion"))
                    .and_then(value_to_f64),
            })
        })
        .collect()
}

pub(crate) fn value_to_f64(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

pub(crate) fn value_to_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

pub(crate) fn value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// Resolves a dotted legacy path such as `data`, `result.models[0]` or `id`.
pub(crate) fn select_path<'a>(
    value: &'a serde_json::Value,
    path: &str,
) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in path.split('.').filter(|segment| !segment.is_empty()) {
        let mut rest = segment;
        while !rest.is_empty() {
            if let Some(start) = rest.find('[') {
                let key = &rest[..start];
                if !key.is_empty() {
                    current = current.get(key)?;
                }
                let end = rest[start + 1..].find(']')? + start + 1;
                let index: usize = rest[start + 1..end].parse().ok()?;
                current = current.get(index)?;
                rest = &rest[end + 1..];
            } else {
                current = current.get(rest)?;
                rest = "";
            }
        }
    }
    Some(current)
}

pub(crate) fn parse_custom_model_list(
    list: &lettuce_models::CustomModelList,
    payload: &serde_json::Value,
) -> Option<Vec<RemoteModel>> {
    let items = select_path(payload, list.list_path.as_str())?.as_array()?;
    let pick = |item: &serde_json::Value, path: Option<&lettuce_models::JsonPath>| {
        path.and_then(|path| select_path(item, path.as_str()))
            .and_then(value_to_string)
            .filter(|text| !text.trim().is_empty())
    };
    let models: Vec<RemoteModel> = items
        .iter()
        .filter_map(|item| {
            let id = select_path(item, list.id_path.as_str()).and_then(value_to_string)?;
            if id.trim().is_empty() {
                return None;
            }
            Some(RemoteModel {
                id,
                display_name: pick(item, list.display_name_path.as_ref()),
                description: pick(item, list.description_path.as_ref()),
                context_length: list
                    .context_length_path
                    .as_ref()
                    .and_then(|path| select_path(item, path.as_str()))
                    .and_then(value_to_u64),
                input_modalities: None,
                output_modalities: None,
                supported_endpoints: None,
                input_price: None,
                output_price: None,
            })
        })
        .collect();
    (!models.is_empty()).then_some(models)
}

pub(crate) fn decode_json(
    response: &lettuce_network::JsonResponse,
) -> Result<serde_json::Value, AdapterError> {
    if let Some(error) = AdapterError::from_response(response) {
        return Err(error);
    }
    serde_json::from_slice(&response.body).map_err(|_| AdapterError::MalformedResponse)
}

pub(crate) use crate::descriptor::RemoteModel;

pub(crate) fn skip_image_data(fragment: &str) -> bool {
    fragment.starts_with("data:image/")
}
