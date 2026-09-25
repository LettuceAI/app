use lettuce_conversations::{InferenceRequest, PortError, ProviderFailure, ProviderFailureKind};
use lettuce_models::{
    PromptCaching, ProviderAccount, ResolvedChatParameters, ResolvedChatProfile, SecretHeader,
};
use lettuce_network::{
    JsonAuth, JsonClientError, JsonSecretHeader, JsonStaticHeader, RequestPolicy, RequestTimeout,
};
use lettuce_settings::{
    HeaderName, SecretOwnerId, SecretPurpose, SecretRef, SecretStore, SecretStoreError, SecretValue,
};

pub(crate) const FALLBACK_MAX_OUTPUT_TOKENS: u32 = 4096;
pub(crate) fn openai_usage_details(
    usage: &serde_json::Map<String, serde_json::Value>,
) -> (Option<u64>, Option<u64>) {
    let first = |value: &serde_json::Map<String, serde_json::Value>, names: &[&str]| {
        names
            .iter()
            .find_map(|name| value.get(*name).and_then(serde_json::Value::as_u64))
    };
    let cached = first(
        usage,
        &[
            "cached_content_token_count",
            "cachedContentTokenCount",
            "cache_read",
            "cacheRead",
        ],
    )
    .or_else(|| {
        usage
            .get("prompt_tokens_details")
            .and_then(serde_json::Value::as_object)
            .and_then(|details| first(details, &["cached_tokens", "cachedTokens"]))
    });
    let reasoning = first(
        usage,
        &[
            "reasoning_tokens",
            "reasoningTokens",
            "thinking_tokens",
            "thinkingTokens",
        ],
    )
    .or_else(|| {
        usage
            .get("completion_tokens_details")
            .and_then(serde_json::Value::as_object)
            .and_then(|details| first(details, &["reasoning_tokens", "reasoningTokens"]))
    });
    (cached, reasoning)
}
pub(crate) fn openai_usage_extras(
    usage: &serde_json::Map<String, serde_json::Value>,
) -> (Option<u64>, Option<u64>) {
    let first = |value: &serde_json::Value, fields: &[&str]| {
        fields
            .iter()
            .find_map(|field| value.get(*field).and_then(serde_json::Value::as_u64))
    };
    (
        usage
            .get("prompt_tokens_details")
            .and_then(|v| first(v, &["cache_write_tokens", "cacheWriteTokens"])),
        usage.get("server_tool_use").and_then(|v| {
            first(
                v,
                &[
                    "web_search_requests",
                    "webSearchRequests",
                    "search_requests",
                ],
            )
        }),
    )
}

/// Image, audio and total token counts as legacy `usage_from_value` and
/// `usage_from_map` read them from an OpenAI `usage` or Gemini
/// `usageMetadata` object (the buffered path also took image tokens from
/// `completion_tokens_details`, so both paths do here). Legacy also
/// counted `prompt_tokens_details.cached_tokens` as image tokens; cached
/// prompt tokens are not image tokens, so that fallback is not kept.
pub(crate) fn usage_modalities(
    usage: &serde_json::Map<String, serde_json::Value>,
) -> (Option<u64>, Option<u64>, Option<u64>) {
    let first = |value: &serde_json::Map<String, serde_json::Value>, names: &[&str]| {
        names
            .iter()
            .find_map(|name| value.get(*name).and_then(serde_json::Value::as_u64))
    };
    let details = |name: &str| usage.get(name).and_then(serde_json::Value::as_object);
    let image = first(usage, &["image_tokens", "imageTokens"])
        .or_else(|| {
            details("prompt_tokens_details")
                .and_then(|value| first(value, &["image_tokens", "imageTokens"]))
        })
        .or_else(|| {
            details("completion_tokens_details")
                .and_then(|value| first(value, &["image_tokens", "imageTokens"]))
        });
    let audio = first(usage, &["audio_tokens", "audioTokens"])
        .or_else(|| {
            details("prompt_tokens_details")
                .and_then(|value| first(value, &["audio_tokens", "audioTokens"]))
        })
        .or_else(|| {
            details("completion_tokens_details")
                .and_then(|value| first(value, &["audio_tokens", "audioTokens"]))
        })
        .or_else(|| modality_token_count(usage.get("promptTokensDetails"), "AUDIO"))
        .or_else(|| modality_token_count(usage.get("candidatesTokensDetails"), "AUDIO"));
    let total = first(usage, &["total_tokens", "totalTokens", "totalTokenCount"]);
    (image, audio, total)
}

/// Legacy `modality_token_count`: the sum of a Gemini modality's entries.
pub(crate) fn modality_token_count(
    details: Option<&serde_json::Value>,
    modality: &str,
) -> Option<u64> {
    let mut total = 0_u64;
    let mut found = false;
    for entry in details?.as_array()? {
        let entry_modality = entry
            .get("modality")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if entry_modality.eq_ignore_ascii_case(modality)
            && let Some(count) = entry
                .get("tokenCount")
                .or_else(|| entry.get("token_count"))
                .and_then(serde_json::Value::as_u64)
        {
            total += count;
            found = true;
        }
    }
    found.then_some(total)
}

pub(crate) fn openai_reported_cost(
    usage: &serde_json::Map<String, serde_json::Value>,
) -> Option<lettuce_conversations::ProviderReportedCost> {
    ["cost", "total_cost", "totalCost"].iter().find_map(|key| {
        let value = usage.get(*key)?;
        let amount = value.as_f64().or_else(|| value.as_str()?.parse().ok())?;
        lettuce_conversations::ProviderReportedCost::new(amount)
    })
}

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

/// Legacy `extract_error_message`: a body that is not JSON is the message
/// itself, and a JSON body without `error`/`message` text (FastAPI
/// `{"detail": ...}` and similar) yields its joined text fragments.
pub(crate) fn provider_error_details(body: &[u8]) -> (Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return (
            None,
            std::str::from_utf8(body)
                .ok()
                .and_then(|text| bounded_diagnostic(text, 2_048)),
        );
    };
    let error = value.get("error").unwrap_or(&value);
    let message = error
        .as_str()
        .or_else(|| error.get("message").and_then(serde_json::Value::as_str))
        .or_else(|| value.get("message").and_then(serde_json::Value::as_str))
        .and_then(|value| bounded_diagnostic(value, 2_048))
        .or_else(|| {
            let mut joined = String::new();
            collect_text_fragments(&value, &mut joined);
            bounded_diagnostic(&joined, 2_048)
        });
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

fn collect_text_fragments(value: &serde_json::Value, joined: &mut String) {
    match value {
        serde_json::Value::String(text) => {
            if !skip_image_data(text) {
                joined.push_str(text);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_text_fragments(item, joined);
            }
        }
        serde_json::Value::Object(map) => {
            if [
                "function_call",
                "functionCall",
                "function_response",
                "functionResponse",
            ]
            .iter()
            .any(|key| map.contains_key(*key))
            {
                return;
            }
            let mut handled = false;
            for key in ["text", "content", "value", "message", "parts"] {
                if let Some(inner) = map.get(key) {
                    handled = true;
                    collect_text_fragments(inner, joined);
                }
            }
            if !handled {
                for inner in map.values() {
                    collect_text_fragments(inner, joined);
                }
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
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

pub(crate) fn validate_common_request_with_tools(
    request: &InferenceRequest,
) -> Result<(), AdapterError> {
    request.validate().map_err(|_| AdapterError::Rejected)?;
    let profile = &request.profile;
    if profile.output_policy != lettuce_conversations::OutputPolicy::Plain {
        return Err(AdapterError::Rejected);
    }
    Ok(())
}

/// The output allowance must be the visible cap plus the reasoning budget,
/// as resolution computes it whether or not reasoning is on.
pub(crate) fn validate_supported_reasoning(
    parameters: &ResolvedChatParameters,
) -> Result<(), AdapterError> {
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
    Ok(())
}

/// Legacy `build_chat_request` applied the caching flag only to providers
/// with explicit caching and read the stored TTL per provider: cache-control
/// providers take `1h` or else five minutes, Gemini `5min` or else one hour,
/// OpenAI `24h` or else in-memory. Every other provider ignored the flag.
pub(crate) fn normalize_prompt_caching(
    support: crate::descriptor::PromptCachingSupport,
    parameters: &mut ResolvedChatParameters,
) {
    use crate::descriptor::PromptCachingSupport;
    use lettuce_models::PromptCacheRetention;
    let Some(PromptCaching::Enabled { retention }) = parameters.prompt_caching else {
        return;
    };
    let retention = match support {
        PromptCachingSupport::None | PromptCachingSupport::Automatic => {
            parameters.prompt_caching = None;
            return;
        }
        PromptCachingSupport::CacheControl => {
            if retention == PromptCacheRetention::OneHour {
                PromptCacheRetention::OneHour
            } else {
                PromptCacheRetention::FiveMinutes
            }
        }
        PromptCachingSupport::ExplicitResource => {
            if retention == PromptCacheRetention::FiveMinutes {
                PromptCacheRetention::FiveMinutes
            } else {
                PromptCacheRetention::OneHour
            }
        }
        PromptCachingSupport::RequestRetention => {
            if retention == PromptCacheRetention::TwentyFourHours {
                PromptCacheRetention::TwentyFourHours
            } else {
                PromptCacheRetention::InMemory
            }
        }
    };
    parameters.prompt_caching = Some(PromptCaching::Enabled { retention });
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
        lettuce_models::ProviderConfig::Standard
        | lettuce_models::ProviderConfig::ComfyUi(_)
        | lettuce_models::ProviderConfig::Ollama(_) => Err(AdapterError::Rejected),
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

/// A field that providers send as `null` as well as omitting it; both mean
/// the default (legacy read these fields with `.as_array()`/`.as_str()`).
pub(crate) fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    Ok(<Option<T> as serde::Deserialize>::deserialize(deserializer)?.unwrap_or_default())
}

/// A token counter read the way legacy `parse_token_value` did: an unsigned
/// integer or an integer string; anything else is unknown, never an error.
pub(crate) fn lenient_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(
        <Option<serde_json::Value> as serde::Deserialize>::deserialize(deserializer)?
            .as_ref()
            .and_then(value_to_u64),
    )
}

/// The provider call id legacy gave a call that arrived without one
/// (`tool_call_{n}`, 1-based within the response).
pub(crate) fn synthesized_call_id(position: usize) -> String {
    format!("tool_call_{}", position + 1)
}

/// Legacy `arguments_value_from_str`: `<parameter=name>value</parameter>`
/// argument strings become an object, JSON objects are kept with their raw
/// text, a JSON string holding an object is unwrapped, and blank or non-object
/// arguments become an empty object (the domain only carries objects; legacy
/// passed the raw string on to the tool).
pub(crate) fn lenient_tool_arguments(raw: &str) -> (serde_json::Value, Option<String>) {
    if let Some(parsed) = parameter_tag_arguments(raw) {
        return (parsed, None);
    }
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value @ serde_json::Value::Object(_)) => (value, Some(raw.to_owned())),
        Ok(serde_json::Value::String(inner)) => {
            match serde_json::from_str::<serde_json::Value>(&inner) {
                Ok(value @ serde_json::Value::Object(_)) => (value, None),
                _ => {
                    if !inner.trim().is_empty() {
                        tracing::warn!("provider tool call arguments are not a JSON object");
                    }
                    (serde_json::Value::Object(serde_json::Map::new()), None)
                }
            }
        }
        _ => {
            if !raw.trim().is_empty() && raw.trim() != "null" {
                tracing::warn!("provider tool call arguments are not a JSON object");
            }
            (serde_json::Value::Object(serde_json::Map::new()), None)
        }
    }
}

/// Arguments that arrived already decoded; `null` or a missing value is an
/// empty object and a string is parsed like a raw argument string.
pub(crate) fn lenient_tool_argument_value(
    value: Option<serde_json::Value>,
) -> (serde_json::Value, Option<String>) {
    match value {
        Some(serde_json::Value::String(raw)) => lenient_tool_arguments(&raw),
        Some(value @ serde_json::Value::Object(_)) => (value, None),
        Some(serde_json::Value::Null) | None => {
            (serde_json::Value::Object(serde_json::Map::new()), None)
        }
        Some(_) => {
            tracing::warn!("provider tool call arguments are not a JSON object");
            (serde_json::Value::Object(serde_json::Map::new()), None)
        }
    }
}

fn parameter_tag_arguments(raw: &str) -> Option<serde_json::Value> {
    let trimmed = raw.trim();
    if !trimmed.contains("<parameter") || !trimmed.contains("</parameter>") {
        return None;
    }
    let mut map = serde_json::Map::new();
    let mut cursor = 0_usize;
    while let Some(start_rel) = trimmed[cursor..].find("<parameter") {
        let start = cursor + start_rel;
        let after_start = &trimmed[start + "<parameter".len()..];
        let Some(name_end) = after_start.find('>') else {
            break;
        };
        let name = after_start[..name_end]
            .trim()
            .trim_start_matches('=')
            .trim_start_matches('-')
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if name.is_empty() {
            break;
        }
        let content_start = start + "<parameter".len() + name_end + 1;
        let Some(end_rel) = trimmed[content_start..].find("</parameter>") else {
            break;
        };
        let content_end = content_start + end_rel;
        map.insert(
            name.to_owned(),
            coerce_parameter_value(&trimmed[content_start..content_end]),
        );
        cursor = content_end + "</parameter>".len();
    }
    (!map.is_empty()).then_some(serde_json::Value::Object(map))
}

fn coerce_parameter_value(raw: &str) -> serde_json::Value {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        return serde_json::Value::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return serde_json::Value::Bool(false);
    }
    if trimmed.eq_ignore_ascii_case("null") {
        return serde_json::Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| serde_json::Value::String(trimmed.to_owned()))
}

/// Legacy `normalize_thinking_content`: the stored reply text and reasoning
/// are trimmed once the response is complete.
pub(crate) fn trim_outcome_text(outcome: &mut lettuce_conversations::InferenceOutcome) {
    for candidate in &mut outcome.candidates {
        candidate.parts.retain_mut(|part| match part {
            lettuce_conversations::MessagePart::Text { text }
            | lettuce_conversations::MessagePart::ReasoningSummary { text } => {
                let trimmed = text.trim();
                if trimmed.len() != text.len() {
                    *text = trimmed.to_owned();
                }
                !text.is_empty()
            }
            _ => true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_reply_text_is_trimmed_like_legacy_normalize_thinking_content() {
        let mut outcome = lettuce_conversations::InferenceOutcome {
            provider_response_id: None,
            candidates: vec![lettuce_conversations::InferenceCandidate {
                ordinal: 0,
                parts: vec![
                    lettuce_conversations::MessagePart::ReasoningSummary {
                        text: "  \n".to_owned(),
                    },
                    lettuce_conversations::MessagePart::Text {
                        text: "\n\nhello \n".to_owned(),
                    },
                ],
                tool_calls: Vec::new(),
                provider_replay: None,
            }],
            usage: None,
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: None,
            warning_codes: Vec::new(),
        };
        trim_outcome_text(&mut outcome);
        assert_eq!(
            outcome.candidates[0].parts,
            vec![lettuce_conversations::MessagePart::Text {
                text: "hello".to_owned()
            }]
        );
    }

    #[test]
    fn prompt_caching_follows_legacy_request_builder_per_provider() {
        use crate::descriptor::PromptCachingSupport as Support;
        use lettuce_models::PromptCacheRetention as Ttl;
        let run = |support, retention| {
            let mut parameters = ResolvedChatParameters {
                prompt_caching: Some(PromptCaching::Enabled { retention }),
                ..crate::integration_tests::parameters()
            };
            normalize_prompt_caching(support, &mut parameters);
            parameters.prompt_caching
        };
        let enabled = |retention| Some(PromptCaching::Enabled { retention });
        assert_eq!(run(Support::None, Ttl::OneHour), None);
        assert_eq!(run(Support::Automatic, Ttl::FiveMinutes), None);
        assert_eq!(
            run(Support::RequestRetention, Ttl::FiveMinutes),
            enabled(Ttl::InMemory)
        );
        assert_eq!(
            run(Support::RequestRetention, Ttl::OneHour),
            enabled(Ttl::InMemory)
        );
        assert_eq!(
            run(Support::RequestRetention, Ttl::TwentyFourHours),
            enabled(Ttl::TwentyFourHours)
        );
        assert_eq!(
            run(Support::CacheControl, Ttl::InMemory),
            enabled(Ttl::FiveMinutes)
        );
        assert_eq!(
            run(Support::CacheControl, Ttl::OneHour),
            enabled(Ttl::OneHour)
        );
        assert_eq!(
            run(Support::ExplicitResource, Ttl::InMemory),
            enabled(Ttl::OneHour)
        );
        assert_eq!(
            run(Support::ExplicitResource, Ttl::FiveMinutes),
            enabled(Ttl::FiveMinutes)
        );
    }

    #[test]
    fn lenient_arguments_unwrap_double_encoded_objects() {
        assert_eq!(
            lenient_tool_arguments(r#""{\"a\":1}""#).0,
            serde_json::json!({"a": 1})
        );
        assert_eq!(
            lenient_tool_arguments(r#"{"a":1}"#),
            (serde_json::json!({"a": 1}), Some(r#"{"a":1}"#.to_owned()))
        );
        assert_eq!(lenient_tool_arguments("not json").0, serde_json::json!({}));
    }
}
