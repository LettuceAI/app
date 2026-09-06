//! Small, policy-bound HTTP transports used by the modular application.

#![deny(unsafe_op_in_unsafe_fn)]

use std::{collections::HashSet, fmt, time::Duration};

use lettuce_settings::{HeaderName, SecretValue};
use reqwest::{Url, header, redirect};
use tokio::time::sleep;

pub const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 256;
const MAX_ENDPOINT_BYTES: usize = 4096;
const MAX_PATH_BYTES: usize = 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const GENERATION_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RETRIES: u32 = 2;
const MAX_RETRY_AFTER: Duration = Duration::from_secs(30);
const REFERER_HEADER: &str = "https://github.com/LettuceAI/";
const TITLE_HEADER: &str = "LettuceAI";

/// A one-shot credential for a JSON POST. It is consumed by the request and
/// is never installed as a client default.
pub enum JsonAuth {
    Bearer(SecretValue),
    Header {
        name: HeaderName,
        value: SecretValue,
    },
    Query {
        name: String,
        value: SecretValue,
    },
    None,
}

impl fmt::Debug for JsonAuth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JsonAuth([REDACTED])")
    }
}

/// An already validated provider-specific secret header.
pub struct JsonSecretHeader {
    pub name: HeaderName,
    pub value: SecretValue,
}

/// A small set of static, non-secret headers supplied for one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonStaticHeader {
    pub name: &'static str,
    pub value: &'static str,
}

/// A non-secret provider-owned query value. Secret query authentication stays
/// in `JsonAuth::Query` so credentials cannot enter reusable URL strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonQueryParameter<'a> {
    pub name: &'a str,
    pub value: &'a str,
}

impl fmt::Debug for JsonSecretHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JsonSecretHeader")
            .field("name", &self.name)
            .field("value", &"[REDACTED]")
            .finish()
    }
}

/// The bounded result of a single HTTP request. Error responses are returned
/// here so provider adapters can classify them without losing the bounded body.
pub struct JsonResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub request_id: Option<String>,
    pub retry_after: Option<String>,
}

/// Bounded ownership of an HTTP response body. Dropping this value closes the
/// request; callers pull one transport chunk at a time so backpressure reaches
/// the socket instead of an unbounded task or channel.
pub struct JsonResponseStream {
    response: reqwest::Response,
    pub status: u16,
    pub request_id: Option<String>,
    pub retry_after: Option<String>,
    received_bytes: usize,
    idle_timeout: Duration,
}

impl fmt::Debug for JsonResponseStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JsonResponseStream")
            .field("status", &self.status)
            .field("request_id", &"[REDACTED]")
            .field("retry_after", &"[REDACTED]")
            .field("received_bytes", &self.received_bytes)
            .finish()
    }
}

impl JsonResponseStream {
    /// Reads the next response chunk with an idle timeout and a cumulative
    /// response-size bound. `None` is a clean end of stream.
    pub async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, JsonClientError> {
        let chunk = tokio::time::timeout(self.idle_timeout, self.response.chunk())
            .await
            .map_err(|_| JsonClientError::Transport)?
            .map_err(|_| JsonClientError::Transport)?;
        let Some(chunk) = chunk else {
            return Ok(None);
        };
        self.received_bytes = self
            .received_bytes
            .checked_add(chunk.len())
            .ok_or(JsonClientError::ResponseTooLarge)?;
        if self.received_bytes > MAX_RESPONSE_BYTES {
            return Err(JsonClientError::ResponseTooLarge);
        }
        Ok(Some(chunk.to_vec()))
    }
}

impl fmt::Debug for JsonResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JsonResponse")
            .field("status", &self.status)
            .field("body", &"[REDACTED]")
            .field("request_id", &"[REDACTED]")
            .field("retry_after", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JsonClientError {
    #[error("HTTP client configuration failed")]
    ClientConfiguration,
    #[error("request URL is invalid")]
    InvalidUrl,
    #[error("request is invalid")]
    InvalidRequest,
    #[error("request body is too large")]
    RequestTooLarge,
    #[error("response body is too large")]
    ResponseTooLarge,
    #[error("HTTP transport failed")]
    Transport,
}

/// Extra PEM roots the user trusts (legacy `appState.trustedCertificates`).
/// Invalid entries are skipped, as the legacy transport did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TlsPolicy {
    pub trusted_roots_pem: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestTimeout {
    /// Legacy 30-minute generation budget.
    Generation,
    /// Legacy 10-second key-verification probe.
    Probe,
}

/// Per-request transport choices owned by the caller's provider policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestPolicy {
    pub timeout: RequestTimeout,
    /// Skips certificate validation. Callers must only set this from an
    /// explicit per-account opt-in, never from a provider default.
    pub allow_invalid_tls: bool,
}

impl RequestPolicy {
    pub const GENERATION: Self = Self {
        timeout: RequestTimeout::Generation,
        allow_invalid_tls: false,
    };
    pub const PROBE: Self = Self {
        timeout: RequestTimeout::Probe,
        allow_invalid_tls: false,
    };
}

/// A concrete, purpose-scoped buffered JSON client (POST and GET).
#[derive(Clone)]
pub struct JsonClient {
    strict: reqwest::Client,
    insecure: reqwest::Client,
}

impl fmt::Debug for JsonClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JsonClient")
    }
}

impl JsonClient {
    pub fn new() -> Result<Self, JsonClientError> {
        Self::with_tls(&TlsPolicy::default())
    }

    pub fn with_tls(policy: &TlsPolicy) -> Result<Self, JsonClientError> {
        let roots: Vec<reqwest::Certificate> = policy
            .trusted_roots_pem
            .iter()
            .filter_map(|pem| reqwest::Certificate::from_pem(pem.as_bytes()).ok())
            .collect();
        Ok(Self {
            strict: build_client(&roots, false)?,
            insecure: build_client(&roots, true)?,
        })
    }

    fn client(&self, policy: RequestPolicy) -> &reqwest::Client {
        if policy.allow_invalid_tls {
            &self.insecure
        } else {
            &self.strict
        }
    }

    /// Sends one logical GET and buffers the JSON (or error) body.
    pub async fn get_json(
        &self,
        endpoint: &str,
        path: &str,
        static_headers: &[JsonStaticHeader],
        auth: JsonAuth,
        secret_headers: Vec<JsonSecretHeader>,
        policy: RequestPolicy,
    ) -> Result<JsonResponse, JsonClientError> {
        self.get_json_with_query(
            endpoint,
            path,
            &[],
            static_headers,
            auth,
            secret_headers,
            policy,
        )
        .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a distinct transport concern; bundling them hides the policy"
    )]
    pub async fn get_json_with_query(
        &self,
        endpoint: &str,
        path: &str,
        query: &[JsonQueryParameter<'_>],
        static_headers: &[JsonStaticHeader],
        auth: JsonAuth,
        secret_headers: Vec<JsonSecretHeader>,
        policy: RequestPolicy,
    ) -> Result<JsonResponse, JsonClientError> {
        let mut url = build_url(endpoint, path)?;
        validate_query(query)?;
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|entry| (entry.name, entry.value)));
        }
        validate_header_collection(static_headers, &auth, &secret_headers)?;
        let request = self.client(policy).get(url).timeout(timeout_for(policy));
        let request = apply_static_headers(request, static_headers)?;
        let request = apply_auth(request, auth)?;
        self.send_with_headers(request, secret_headers, retries_for(policy))
            .await
    }

    /// Sends one logical request. Server errors, rate limits with a short
    /// `Retry-After`, timeouts, and connection failures are retried a bounded
    /// number of times with backoff. Dropping the returned future is the only
    /// cancellation mechanism; no detached task is created here.
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a distinct transport concern; bundling them hides the policy"
    )]
    pub async fn post_json(
        &self,
        endpoint: &str,
        path: &str,
        body: Vec<u8>,
        static_headers: &[JsonStaticHeader],
        auth: JsonAuth,
        secret_headers: Vec<JsonSecretHeader>,
        policy: RequestPolicy,
    ) -> Result<JsonResponse, JsonClientError> {
        if body.len() > MAX_REQUEST_BYTES {
            return Err(JsonClientError::RequestTooLarge);
        }
        let url = build_url(endpoint, path)?;
        validate_header_collection(static_headers, &auth, &secret_headers)?;
        let request = self
            .client(policy)
            .post(url)
            .timeout(timeout_for(policy))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body);
        let request = apply_static_headers(request, static_headers)?;
        let request = apply_auth(request, auth)?;
        self.send_with_headers(request, secret_headers, retries_for(policy))
            .await
    }

    /// Starts a JSON POST while leaving the response body attached to the
    /// caller. Retries only happen before any body bytes become observable.
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a distinct transport concern; bundling them hides the policy"
    )]
    pub async fn post_json_stream(
        &self,
        endpoint: &str,
        path: &str,
        body: Vec<u8>,
        static_headers: &[JsonStaticHeader],
        auth: JsonAuth,
        secret_headers: Vec<JsonSecretHeader>,
        policy: RequestPolicy,
    ) -> Result<JsonResponseStream, JsonClientError> {
        self.post_json_stream_with_query(
            endpoint,
            path,
            body,
            &[],
            static_headers,
            auth,
            secret_headers,
            policy,
        )
        .await
    }

    /// Starts a streaming JSON POST with a bounded set of non-secret static
    /// query parameters used by provider protocols such as Gemini SSE.
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a distinct transport concern; bundling them hides the policy"
    )]
    pub async fn post_json_stream_with_query(
        &self,
        endpoint: &str,
        path: &str,
        body: Vec<u8>,
        query: &[JsonQueryParameter<'_>],
        static_headers: &[JsonStaticHeader],
        auth: JsonAuth,
        secret_headers: Vec<JsonSecretHeader>,
        policy: RequestPolicy,
    ) -> Result<JsonResponseStream, JsonClientError> {
        if body.len() > MAX_REQUEST_BYTES {
            return Err(JsonClientError::RequestTooLarge);
        }
        let mut url = build_url(endpoint, path)?;
        validate_query(query)?;
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|entry| (entry.name, entry.value)));
        }
        validate_header_collection(static_headers, &auth, &secret_headers)?;
        let request = self
            .client(policy)
            .post(url)
            .timeout(timeout_for(policy))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body);
        let request = apply_static_headers(request, static_headers)?;
        let request = apply_auth(request, auth)?;
        let request = apply_secret_headers(request, secret_headers)?;
        self.send_stream(request, retries_for(policy), timeout_for(policy))
            .await
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        max_retries: u32,
    ) -> Result<JsonResponse, JsonClientError> {
        let mut attempt = 0_u32;
        loop {
            let current = request.try_clone().ok_or(JsonClientError::InvalidRequest)?;
            match current.send().await {
                Ok(response) => {
                    let delay = (attempt < max_retries)
                        .then(|| retry_delay_for_status(&response, attempt + 1))
                        .flatten();
                    match delay {
                        Some(delay) => {
                            attempt += 1;
                            sleep(delay).await;
                        }
                        None => return read_response(response).await,
                    }
                }
                Err(error) => {
                    if attempt < max_retries && (error.is_timeout() || error.is_request()) {
                        attempt += 1;
                        sleep(backoff_delay(attempt)).await;
                    } else {
                        return Err(JsonClientError::Transport);
                    }
                }
            }
        }
    }

    async fn send_stream(
        &self,
        request: reqwest::RequestBuilder,
        max_retries: u32,
        idle_timeout: Duration,
    ) -> Result<JsonResponseStream, JsonClientError> {
        let mut attempt = 0_u32;
        loop {
            let current = request.try_clone().ok_or(JsonClientError::InvalidRequest)?;
            match current.send().await {
                Ok(response) => {
                    let delay = (attempt < max_retries)
                        .then(|| retry_delay_for_status(&response, attempt + 1))
                        .flatten();
                    if let Some(delay) = delay {
                        attempt += 1;
                        sleep(delay).await;
                        continue;
                    }
                    return response_stream(response, idle_timeout);
                }
                Err(error) => {
                    if attempt < max_retries && (error.is_timeout() || error.is_request()) {
                        attempt += 1;
                        sleep(backoff_delay(attempt)).await;
                    } else {
                        return Err(JsonClientError::Transport);
                    }
                }
            }
        }
    }
    async fn send_with_headers(
        &self,
        request: reqwest::RequestBuilder,
        secret_headers: Vec<JsonSecretHeader>,
        max_retries: u32,
    ) -> Result<JsonResponse, JsonClientError> {
        let request = apply_secret_headers(request, secret_headers)?;
        self.send(request, max_retries).await
    }
}

fn response_stream(
    response: reqwest::Response,
    idle_timeout: Duration,
) -> Result<JsonResponseStream, JsonClientError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(JsonClientError::ResponseTooLarge);
    }
    Ok(JsonResponseStream {
        status: response.status().as_u16(),
        request_id: bounded_header(&response, "x-request-id")
            .or_else(|| bounded_header(&response, "request-id")),
        retry_after: bounded_header(&response, "retry-after"),
        response,
        received_bytes: 0,
        idle_timeout,
    })
}

fn apply_secret_headers(
    mut request: reqwest::RequestBuilder,
    secret_headers: Vec<JsonSecretHeader>,
) -> Result<reqwest::RequestBuilder, JsonClientError> {
    for secret_header in secret_headers {
        let header_name = to_header_name(&secret_header.name)?;
        let header_value = secret_header
            .value
            .with(header::HeaderValue::from_str)
            .map_err(|_| JsonClientError::InvalidRequest)?;
        request = sensitive_header(request, header_name, header_value);
    }
    Ok(request)
}

fn build_client(
    roots: &[reqwest::Certificate],
    accept_invalid: bool,
) -> Result<reqwest::Client, JsonClientError> {
    let mut default_headers = header::HeaderMap::with_capacity(2);
    default_headers.insert(
        "http-referer",
        header::HeaderValue::from_static(REFERER_HEADER),
    );
    default_headers.insert("x-title", header::HeaderValue::from_static(TITLE_HEADER));
    let mut builder = reqwest::Client::builder()
        .redirect(redirect::Policy::none())
        .referer(false)
        .no_proxy()
        .default_headers(default_headers)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(GENERATION_TIMEOUT)
        .danger_accept_invalid_certs(accept_invalid);
    for root in roots {
        builder = builder.add_root_certificate(root.clone());
    }
    builder
        .build()
        .map_err(|_| JsonClientError::ClientConfiguration)
}

/// Legacy verification probes used a bare client with no retry loop.
fn retries_for(policy: RequestPolicy) -> u32 {
    match policy.timeout {
        RequestTimeout::Generation => MAX_RETRIES,
        RequestTimeout::Probe => 0,
    }
}

fn timeout_for(policy: RequestPolicy) -> Duration {
    match policy.timeout {
        RequestTimeout::Generation => GENERATION_TIMEOUT,
        RequestTimeout::Probe => PROBE_TIMEOUT,
    }
}

fn apply_auth(
    request: reqwest::RequestBuilder,
    auth: JsonAuth,
) -> Result<reqwest::RequestBuilder, JsonClientError> {
    Ok(match auth {
        JsonAuth::Bearer(value) => {
            let header_value = value
                .with(|secret| header::HeaderValue::from_str(&format!("Bearer {secret}")))
                .map_err(|_| JsonClientError::InvalidRequest)?;
            sensitive_header(request, header::AUTHORIZATION, header_value)
        }
        JsonAuth::Header { name, value } => {
            let header_name = to_header_name(&name)?;
            let header_value = value
                .with(header::HeaderValue::from_str)
                .map_err(|_| JsonClientError::InvalidRequest)?;
            sensitive_header(request, header_name, header_value)
        }
        JsonAuth::Query { name, value } => {
            if name.is_empty()
                || name.len() > 128
                || name.chars().any(|character| character.is_control())
            {
                return Err(JsonClientError::InvalidRequest);
            }
            value.with(|secret| request.query(&[(name.as_str(), secret)]))
        }
        JsonAuth::None => request,
    })
}

async fn read_response(response: reqwest::Response) -> Result<JsonResponse, JsonClientError> {
    let status = response.status().as_u16();
    let request_id = bounded_header(&response, "x-request-id")
        .or_else(|| bounded_header(&response, "request-id"));
    let retry_after = bounded_header(&response, "retry-after");
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(JsonClientError::ResponseTooLarge);
    }
    let mut body = Vec::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| JsonClientError::Transport)?
    {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(JsonClientError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(JsonResponse {
        status,
        body,
        request_id,
        retry_after,
    })
}

fn retry_delay_for_status(response: &reqwest::Response, attempt: u32) -> Option<Duration> {
    let status = response.status();
    if status.as_u16() == 429 {
        match bounded_header(response, "retry-after").and_then(|value| value.parse::<u64>().ok()) {
            Some(seconds) => {
                let delay = Duration::from_secs(seconds);
                (delay <= MAX_RETRY_AFTER).then_some(delay)
            }
            None => Some(backoff_delay(attempt)),
        }
    } else if status.is_server_error() {
        Some(backoff_delay(attempt))
    } else {
        None
    }
}

fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis(200 * (1_u64 << attempt.saturating_sub(1).min(3)))
}

fn validate_header_collection(
    static_headers: &[JsonStaticHeader],
    auth: &JsonAuth,
    secret_headers: &[JsonSecretHeader],
) -> Result<(), JsonClientError> {
    let mut names = HashSet::with_capacity(static_headers.len() + secret_headers.len() + 1);
    names.insert("content-type".to_owned());
    let auth_name = match auth {
        JsonAuth::Bearer(_) => Some("authorization".to_owned()),
        JsonAuth::Header { name, .. } => {
            if is_extra_reserved_header(name.as_str()) {
                return Err(JsonClientError::InvalidRequest);
            }
            Some(name.as_str().to_ascii_lowercase())
        }
        JsonAuth::Query { .. } | JsonAuth::None => None,
    };
    if let Some(auth_name) = &auth_name {
        names.insert(auth_name.clone());
    }
    for header in static_headers {
        let name = header_name_from_static(header.name)?;
        let normalized = name.as_str().to_ascii_lowercase();
        if is_extra_reserved_header(name.as_str())
            || !names.insert(normalized)
            || header::HeaderValue::from_str(header.value).is_err()
        {
            return Err(JsonClientError::InvalidRequest);
        }
    }
    for header in secret_headers {
        let normalized = header.name.as_str().to_ascii_lowercase();
        if is_extra_reserved_header(header.name.as_str()) || !names.insert(normalized) {
            return Err(JsonClientError::InvalidRequest);
        }
    }
    Ok(())
}

fn apply_static_headers(
    mut request: reqwest::RequestBuilder,
    static_headers: &[JsonStaticHeader],
) -> Result<reqwest::RequestBuilder, JsonClientError> {
    for header in static_headers {
        request = request.header(
            header_name_from_static(header.name)?,
            header::HeaderValue::from_str(header.value)
                .map_err(|_| JsonClientError::InvalidRequest)?,
        );
    }
    Ok(request)
}

fn header_name_from_static(name: &'static str) -> Result<header::HeaderName, JsonClientError> {
    name.parse().map_err(|_| JsonClientError::InvalidRequest)
}

fn sensitive_header(
    request: reqwest::RequestBuilder,
    name: header::HeaderName,
    mut value: header::HeaderValue,
) -> reqwest::RequestBuilder {
    value.set_sensitive(true);
    request.header(name, value)
}

fn to_header_name(name: &HeaderName) -> Result<header::HeaderName, JsonClientError> {
    name.as_str()
        .parse()
        .map_err(|_| JsonClientError::InvalidRequest)
}

fn is_transport_reserved_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "host"
            | "content-length"
            | "content-type"
            | "transfer-encoding"
            | "connection"
            | "upgrade"
            | "keep-alive"
            | "te"
            | "trailer"
            | "proxy-authorization"
            | "proxy-connection"
    )
}

fn is_extra_reserved_header(name: &str) -> bool {
    is_transport_reserved_header(name)
        || matches!(
            name.to_ascii_lowercase().as_str(),
            "authorization" | "http-referer" | "x-title"
        )
}

fn validate_query(query: &[JsonQueryParameter<'_>]) -> Result<(), JsonClientError> {
    if query.len() > 16 {
        return Err(JsonClientError::InvalidRequest);
    }
    let mut names = HashSet::with_capacity(query.len());
    for entry in query {
        if entry.name.is_empty()
            || entry.name.len() > 128
            || entry.value.len() > 256
            || entry.name.chars().any(char::is_control)
            || entry.value.chars().any(char::is_control)
            || !names.insert(entry.name)
        {
            return Err(JsonClientError::InvalidRequest);
        }
    }
    Ok(())
}

fn build_url(endpoint: &str, path: &str) -> Result<Url, JsonClientError> {
    if endpoint.trim() != endpoint
        || endpoint.is_empty()
        || endpoint.len() > MAX_ENDPOINT_BYTES
        || endpoint
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || path.trim() != path
        || path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || !path.starts_with('/')
        || path.starts_with("//")
        || path.contains("//")
        || path.contains(['?', '#'])
        || path
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(JsonClientError::InvalidUrl);
    }
    let authority_start = endpoint.find("://").map_or(0, |index| index + 3);
    let raw_path_start = endpoint[authority_start..]
        .find('/')
        .map_or(endpoint.len(), |offset| authority_start + offset);
    if !safe_path(&endpoint[raw_path_start..]) {
        return Err(JsonClientError::InvalidUrl);
    }
    let mut url = Url::parse(endpoint).map_err(|_| JsonClientError::InvalidUrl)?;
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(JsonClientError::InvalidUrl);
    }
    url.host_str().ok_or(JsonClientError::InvalidUrl)?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || endpoint_authority_contains_userinfo(endpoint)
    {
        return Err(JsonClientError::InvalidUrl);
    }
    if !safe_path(url.path()) || !safe_path(path) {
        return Err(JsonClientError::InvalidUrl);
    }
    let base_path = url.path().trim_end_matches('/');
    let chat_path = path.trim_start_matches('/');
    let joined_path = if base_path.is_empty() {
        format!("/{chat_path}")
    } else if chat_path.is_empty() {
        format!("{base_path}/")
    } else {
        format!("{base_path}/{chat_path}")
    };
    url.set_path(&joined_path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn safe_path(path: &str) -> bool {
    let lowered = path.to_ascii_lowercase();
    !path.contains('\\')
        && !lowered.contains("%2f")
        && !lowered.contains("%5c")
        && !lowered
            .split('/')
            .any(|segment| segment == "." || segment == ".." || segment.contains("%2e"))
}

fn endpoint_authority_contains_userinfo(endpoint: &str) -> bool {
    let Some(authority_start) = endpoint.find("://").map(|index| index + 3) else {
        return true;
    };
    let authority_end = endpoint[authority_start..]
        .find('/')
        .map_or(endpoint.len(), |offset| authority_start + offset);
    endpoint[authority_start..authority_end].contains('@')
}

fn bounded_header(response: &reqwest::Response, name: &str) -> Option<String> {
    let value = response.headers().get(name)?.to_str().ok()?;
    if value.len() <= MAX_METADATA_BYTES && !value.chars().any(|character| character.is_control()) {
        Some(value.to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    async fn test_server(response: &'static str) -> (String, oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("server address");
        let (sender, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            let body_start = loop {
                let read = stream.read(&mut buffer).await.expect("read request");
                if read == 0 {
                    break None;
                }
                request.extend_from_slice(&buffer[..read]);
                if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    break Some(position + 4);
                }
            };
            if let Some(body_start) = body_start {
                let headers = String::from_utf8_lossy(&request[..body_start]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("Content-Length:")
                            .or_else(|| line.strip_prefix("content-length:"))
                    })
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                while request.len() < body_start + content_length {
                    let read = stream.read(&mut buffer).await.expect("read body");
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                }
            }
            let _ = sender.send(request);
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        });
        (format!("http://{address}"), receiver)
    }

    async fn sequence_server(
        responses: Vec<&'static str>,
    ) -> (String, oneshot::Receiver<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("server address");
        let (sender, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let mut requests = Vec::with_capacity(responses.len());
            for response in responses {
                let (mut stream, _) = listener.accept().await.expect("accept request");
                let mut request = Vec::new();
                let mut buffer = [0_u8; 4096];
                loop {
                    let read = stream.read(&mut buffer).await.expect("read request");
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                requests.push(String::from_utf8_lossy(&request).into_owned());
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("write response");
            }
            let _ = sender.send(requests);
        });
        (format!("http://{address}"), receiver)
    }

    fn client() -> JsonClient {
        JsonClient::new().expect("client")
    }

    #[tokio::test]
    async fn streaming_post_preserves_metadata_and_bounds_pull_based_chunks() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind stream server");
        let endpoint = format!("http://{}", listener.local_addr().expect("address"));
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept request");
            let mut request = vec![0_u8; 2_048];
            let _ = socket.read(&mut request).await.expect("read request");
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nX-Request-Id: stream-canary\r\nTransfer-Encoding: chunked\r\n\r\n",
                )
                .await
                .expect("headers");
            for chunk in [
                b"D\r\ndata: first\n\n\r\n".as_slice(),
                b"E\r\ndata: second\n\n\r\n".as_slice(),
            ] {
                socket.write_all(chunk).await.expect("stream chunk");
                tokio::task::yield_now().await;
            }
            socket.write_all(b"0\r\n\r\n").await.expect("stream end");
        });

        let mut response = client()
            .post_json_stream(
                &endpoint,
                "/chat",
                b"{}".to_vec(),
                &[],
                JsonAuth::None,
                Vec::new(),
                RequestPolicy::GENERATION,
            )
            .await
            .expect("stream response");
        assert_eq!(response.status, 200);
        assert_eq!(response.request_id.as_deref(), Some("stream-canary"));
        let mut body = Vec::new();
        while let Some(chunk) = response.next_chunk().await.expect("next chunk") {
            body.extend_from_slice(&chunk);
        }
        assert_eq!(body, b"data: first\n\ndata: second\n\n");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn retries_server_errors_and_short_rate_limits_then_succeeds() {
        let (endpoint, requests) = sequence_server(vec![
            "HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n\r\n{}",
        ])
        .await;
        let response = client()
            .post_json(
                &endpoint,
                "/chat",
                b"{}".to_vec(),
                &[],
                JsonAuth::None,
                Vec::new(),
                RequestPolicy::GENERATION,
            )
            .await
            .expect("response after retries");
        assert_eq!(response.status, 200);
        let requests = requests.await.expect("requests received");
        assert_eq!(requests.len(), 3);
        for request in &requests {
            let lower = request.to_ascii_lowercase();
            assert!(lower.contains("http-referer: https://github.com/lettuceai/\r\n"));
            assert!(lower.contains("x-title: lettuceai\r\n"));
        }
    }

    #[tokio::test]
    async fn stops_retrying_after_the_bounded_attempts_and_long_retry_after() {
        let (endpoint, requests) = sequence_server(vec![
            "HTTP/1.1 500 Internal Server Error\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        ])
        .await;
        let response = client()
            .post_json(
                &endpoint,
                "/chat",
                b"{}".to_vec(),
                &[],
                JsonAuth::None,
                Vec::new(),
                RequestPolicy::GENERATION,
            )
            .await
            .expect("final server error response");
        assert_eq!(response.status, 503);
        assert_eq!(requests.await.expect("requests received").len(), 3);

        let (endpoint, requests) = sequence_server(vec![
            "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 3600\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        ])
        .await;
        let response = client()
            .post_json(
                &endpoint,
                "/chat",
                b"{}".to_vec(),
                &[],
                JsonAuth::None,
                Vec::new(),
                RequestPolicy::GENERATION,
            )
            .await
            .expect("long retry-after is returned, not slept on");
        assert_eq!(response.status, 429);
        assert_eq!(response.retry_after.as_deref(), Some("3600"));
        assert_eq!(requests.await.expect("requests received").len(), 1);
    }

    #[tokio::test]
    async fn posts_exact_json_and_percent_encodes_query_credentials() {
        let (endpoint, request) =
            test_server("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").await;
        let accept = JsonStaticHeader {
            name: "accept",
            value: "application/json",
        };
        let response = client()
            .post_json(
                &endpoint,
                "/v1/chat/completions",
                br#"{"model":"canary"}"#.to_vec(),
                &[accept],
                JsonAuth::Query {
                    name: "api-key".to_owned(),
                    value: SecretValue::new("secret+canary&=").expect("secret"),
                },
                vec![JsonSecretHeader {
                    name: HeaderName::new("x-extra").expect("header"),
                    value: SecretValue::new("header-canary").expect("secret"),
                }],
                RequestPolicy::GENERATION,
            )
            .await
            .expect("response");
        assert_eq!(response.status, 200);
        let request = String::from_utf8(request.await.expect("request received")).expect("HTTP");
        assert!(
            request.starts_with("POST /v1/chat/completions?api-key=secret%2Bcanary%26%3D HTTP/1.1")
        );
        let request_lower = request.to_ascii_lowercase();
        assert!(request_lower.contains("content-type: application/json\r\n"));
        assert!(request_lower.contains("accept: application/json\r\n"));
        assert!(request_lower.contains("x-extra: header-canary\r\n"));
        assert!(request.ends_with(r#"{"model":"canary"}"#));
    }

    #[tokio::test]
    async fn preserves_bounded_error_body_and_does_not_follow_redirects() {
        let (endpoint, request) = test_server(
            "HTTP/1.1 401 Unauthorized\r\nX-Request-Id: request-canary\r\nRetry-After: 4\r\nContent-Length: 17\r\n\r\nerror-canary-body",
        )
        .await;
        let response = client()
            .post_json(
                &endpoint,
                "/chat",
                b"{}".to_vec(),
                &[],
                JsonAuth::Bearer(SecretValue::new("token-canary").expect("secret")),
                Vec::new(),
                RequestPolicy::GENERATION,
            )
            .await
            .expect("response");
        assert_eq!(response.status, 401);
        assert_eq!(response.body, b"error-canary-body");
        assert_eq!(response.request_id.as_deref(), Some("request-canary"));
        assert_eq!(response.retry_after.as_deref(), Some("4"));
        let request = String::from_utf8(request.await.expect("request received")).expect("HTTP");
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer token-canary\r\n")
        );

        let (endpoint, _) = test_server(
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/other\r\nContent-Length: 0\r\n\r\n",
        )
        .await;
        let response = client()
            .post_json(
                &endpoint,
                "/redirect",
                b"{}".to_vec(),
                &[],
                JsonAuth::None,
                Vec::new(),
                RequestPolicy::GENERATION,
            )
            .await
            .expect("redirect response");
        assert_eq!(response.status, 302);
    }

    #[tokio::test]
    async fn rejects_response_over_the_buffer_limit_and_redacts_debug() {
        let (endpoint, _) = test_server("HTTP/1.1 200 OK\r\nContent-Length: 8388609\r\n\r\n").await;
        let error = client()
            .post_json(
                &endpoint,
                "/large",
                b"{}".to_vec(),
                &[],
                JsonAuth::None,
                Vec::new(),
                RequestPolicy::GENERATION,
            )
            .await
            .expect_err("oversized response");
        assert_eq!(error, JsonClientError::ResponseTooLarge);
        assert!(!format!("{error:?}").contains("canary"));
        let auth = JsonAuth::Bearer(SecretValue::new("secret-canary").expect("secret"));
        assert!(!format!("{auth:?}").contains("secret-canary"));
        let response = JsonResponse {
            status: 200,
            body: b"prompt-canary".to_vec(),
            request_id: Some("header-canary".to_owned()),
            retry_after: Some("retry-canary".to_owned()),
        };
        let debug = format!("{response:?}");
        assert!(!debug.contains("prompt-canary"));
        assert!(!debug.contains("header-canary"));
        assert!(!debug.contains("retry-canary"));
    }

    #[test]
    fn allows_plain_http_hosts_and_rejects_userinfo_and_encoded_separators() {
        assert_eq!(
            build_url("http://192.168.1.20:1234", "/v1/chat/completions")
                .expect("LAN http endpoint")
                .as_str(),
            "http://192.168.1.20:1234/v1/chat/completions"
        );
        assert_eq!(
            build_url("https://user:pass@example.com", "/chat"),
            Err(JsonClientError::InvalidUrl)
        );
        assert_eq!(
            build_url("https://example.com", "/chat%2F..%2Fadmin"),
            Err(JsonClientError::InvalidUrl)
        );
        assert_eq!(
            build_url("http://localhost", "/chat")
                .expect("loopback")
                .path(),
            "/chat"
        );
        assert_eq!(
            build_url("https://example.com/v1", "/chat")
                .expect("base path join")
                .path(),
            "/v1/chat"
        );
        assert_eq!(
            build_url("https://example.com/v1/", "/chat")
                .expect("trim base slash")
                .path(),
            "/v1/chat"
        );
        assert_eq!(
            build_url("https://example.com/v1", "/../chat"),
            Err(JsonClientError::InvalidUrl)
        );
        assert_eq!(
            build_url("https://example.com/v1/../", "/chat"),
            Err(JsonClientError::InvalidUrl)
        );
    }

    #[tokio::test]
    async fn rejects_static_and_secret_header_collisions_before_send() {
        let client = client();
        let auth_name = HeaderName::new("x-auth").expect("header");
        let error = client
            .post_json(
                "http://127.0.0.1:1",
                "/chat",
                b"{}".to_vec(),
                &[],
                JsonAuth::Header {
                    name: auth_name.clone(),
                    value: SecretValue::new("auth-canary").expect("secret"),
                },
                vec![JsonSecretHeader {
                    name: auth_name,
                    value: SecretValue::new("duplicate-canary").expect("secret"),
                }],
                RequestPolicy::GENERATION,
            )
            .await
            .expect_err("auth/header collision");
        assert_eq!(error, JsonClientError::InvalidRequest);

        let accept = JsonStaticHeader {
            name: "accept",
            value: "application/json",
        };
        let error = client
            .post_json(
                "http://127.0.0.1:1",
                "/chat",
                b"{}".to_vec(),
                &[accept],
                JsonAuth::None,
                vec![JsonSecretHeader {
                    name: HeaderName::new("Accept").expect("header"),
                    value: SecretValue::new("duplicate-canary").expect("secret"),
                }],
                RequestPolicy::GENERATION,
            )
            .await
            .expect_err("static/header collision");
        assert_eq!(error, JsonClientError::InvalidRequest);

        let error = client
            .post_json(
                "http://127.0.0.1:1",
                "/chat",
                b"{}".to_vec(),
                &[],
                JsonAuth::None,
                vec![JsonSecretHeader {
                    name: HeaderName::new("X-Title").expect("header"),
                    value: SecretValue::new("duplicate-canary").expect("secret"),
                }],
                RequestPolicy::GENERATION,
            )
            .await
            .expect_err("client default header collision");
        assert_eq!(error, JsonClientError::InvalidRequest);
    }
}
