//! Remote image generation (legacy `generate_image` for every provider but
//! sdcpp): one request per generation, never retried, with the messages
//! legacy showed.

mod adapters;
mod comfyui;
#[cfg(test)]
mod tests;
mod usage;

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use lettuce_image_generation::{
    ImageProviderError, ImageProviderPort, ProviderImage, ProviderImageOutput, ProviderImageRequest,
};
use lettuce_network::{BulkHttpClient, JsonAuth, JsonResponse, JsonStaticHeader};
use lettuce_settings::{HeaderName, SecretStore, SecretValue};
use serde_json::Value;

use crate::common::{Credentials, load_secret_headers, tls_opt_in_allowed};
use adapters::{ImageAuth, ImagePayload, ImageResponseData, adapter_for, default_base_url};

const MISSING_API_KEY: &str = "API key not found for provider";

/// The remote image providers legacy supported, over one secret store.
pub struct RemoteImageProviders<S: ?Sized> {
    secret_store: Arc<S>,
    http: BulkHttpClient,
}

impl<S: ?Sized> std::fmt::Debug for RemoteImageProviders<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RemoteImageProviders")
    }
}

impl<S: SecretStore + ?Sized> RemoteImageProviders<S> {
    #[must_use]
    pub fn new(secret_store: Arc<S>, http: BulkHttpClient) -> Self {
        Self { secret_store, http }
    }

    async fn api_key(
        &self,
        request: &ProviderImageRequest,
    ) -> Result<Option<SecretValue>, ImageProviderError> {
        let Some(reference) = request.account.api_key_ref else {
            return Ok(None);
        };
        self.secret_store
            .load(
                &reference,
                &lettuce_settings::SecretPurpose::ProviderApiKey {
                    owner: request.account.secret_owner_id,
                },
            )
            .await
            .map(Some)
            .map_err(|_| ImageProviderError::Failed(MISSING_API_KEY.to_owned()))
    }

    async fn secret_headers(
        &self,
        request: &ProviderImageRequest,
    ) -> Result<Vec<lettuce_network::JsonSecretHeader>, ImageProviderError> {
        load_secret_headers(
            &*self.secret_store,
            &Credentials {
                owner: request.account.secret_owner_id,
                api_key_ref: request.account.api_key_ref,
                secret_headers: &request.account.secret_headers,
                allow_invalid_tls: false,
            },
        )
        .await
        .map_err(|_| ImageProviderError::Failed("Provider headers could not be read".to_owned()))
    }
}

/// Legacy `resolve_base_url`: the account's own URL without trailing
/// slashes, else the catalog default. `custom` and `lettuce-host` have no
/// default (legacy's was empty, so the request could not be sent).
fn base_url(request: &ProviderImageRequest, kind: &str) -> Result<String, ImageProviderError> {
    match request.account.endpoint.as_deref() {
        Some(custom) if !custom.is_empty() => Ok(custom.trim_end_matches('/').to_owned()),
        _ if matches!(kind, "custom" | "lettuce-host") => Err(ImageProviderError::Failed(
            "Request failed: the provider has no base URL".to_owned(),
        )),
        _ => Ok(default_base_url(kind).to_owned()),
    }
}

/// Legacy allowed the invalid-TLS opt-in for the editable-endpoint text
/// providers and for Automatic1111.
fn allow_invalid_tls(request: &ProviderImageRequest, kind: &str) -> bool {
    request.account.allow_invalid_tls && (tls_opt_in_allowed(kind) || kind == "automatic1111")
}

fn api_error(response: &JsonResponse) -> ImageProviderError {
    ImageProviderError::Failed(format!(
        "API error {}: {}",
        lettuce_network::status_text(response.status),
        String::from_utf8_lossy(&response.body)
    ))
}

fn transport_error(error: impl std::fmt::Display) -> ImageProviderError {
    ImageProviderError::Failed(format!("Request failed: {error}"))
}

/// Legacy `save_image`: `data:image...` URLs and raw base64 decode here;
/// HTTP(S) URLs are fetched.
async fn image_bytes(
    http: &BulkHttpClient,
    source: &str,
) -> Result<(Vec<u8>, Option<String>), ImageProviderError> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let response = http.fetch_url(source).await.map_err(|error| {
            ImageProviderError::Failed(format!("Failed to download image: {error}"))
        })?;
        if !(200..300).contains(&response.status) {
            return Err(ImageProviderError::Failed(format!(
                "Failed to download image: HTTP {}",
                lettuce_network::status_text(response.status)
            )));
        }
        return Ok((response.body, None));
    }
    let (declared, encoded) = if source.starts_with("data:image") {
        let (prefix, data) = source
            .split_once(',')
            .ok_or_else(|| ImageProviderError::Failed("Invalid data URL format".to_owned()))?;
        (
            prefix
                .strip_prefix("data:")
                .and_then(|value| value.split(';').next())
                .map(str::to_owned),
            data,
        )
    } else {
        (None, source)
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|error| ImageProviderError::Failed(format!("Failed to decode base64: {error}")))?;
    Ok((bytes, declared))
}

async fn collect_images(
    http: &BulkHttpClient,
    items: Vec<ImageResponseData>,
) -> Result<Vec<ProviderImage>, ImageProviderError> {
    let mut images = Vec::with_capacity(items.len());
    for item in items {
        let Some(source) = item.url.as_ref().or(item.b64_json.as_ref()) else {
            let detail = item
                .text
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| {
                    format!(
                        " Provider returned text instead: {}",
                        text.chars().take(160).collect::<String>()
                    )
                })
                .unwrap_or_default();
            return Err(ImageProviderError::Failed(format!(
                "No image URL or data in response.{detail}"
            )));
        };
        let (bytes, declared_mime_type) = image_bytes(http, source).await?;
        images.push(ProviderImage {
            bytes,
            declared_mime_type,
            text: item.text,
        });
    }
    Ok(images)
}

#[async_trait]
impl<S: SecretStore + ?Sized> ImageProviderPort for RemoteImageProviders<S> {
    async fn generate(
        &self,
        request: ProviderImageRequest,
    ) -> Result<ProviderImageOutput, ImageProviderError> {
        let cancellation = request.cancellation.clone();
        if cancellation.is_cancelled() {
            return Err(ImageProviderError::Cancelled);
        }
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(ImageProviderError::Cancelled),
            result = self.run(request) => result,
        }
    }
}

impl<S: SecretStore + ?Sized> RemoteImageProviders<S> {
    async fn run(
        &self,
        request: ProviderImageRequest,
    ) -> Result<ProviderImageOutput, ImageProviderError> {
        let kind = request.account.provider_kind.to_ascii_lowercase();
        let api_key = self.api_key(&request).await?;
        let base = base_url(&request, &kind)?;
        let insecure = allow_invalid_tls(&request, &kind);
        if kind == "comfyui" {
            let items =
                comfyui::generate(&self.http, &request, &base, api_key.as_ref(), insecure).await?;
            return Ok(ProviderImageOutput {
                images: collect_images(&self.http, items).await?,
                usage: None,
            });
        }
        let adapter =
            adapter_for(&kind).ok_or_else(|| ImageProviderError::Unsupported(kind.clone()))?;
        if adapter.requires_api_key() && api_key.is_none() {
            return Err(ImageProviderError::Failed(MISSING_API_KEY.to_owned()));
        }
        let secret_headers = self.secret_headers(&request).await?;
        let call = adapter
            .call(&base, &request)
            .map_err(ImageProviderError::Failed)?;
        let auth = match (call.auth, api_key) {
            (ImageAuth::Bearer, Some(key)) => JsonAuth::Bearer(key),
            (ImageAuth::OptionalBearer, Some(key)) if !key.with(str::is_empty) => {
                JsonAuth::Bearer(key)
            }
            (ImageAuth::GoogleApiKeyHeader, Some(key)) => JsonAuth::Header {
                name: HeaderName::new("x-goog-api-key").expect("static header name"),
                value: key,
            },
            (ImageAuth::QueryKey, Some(key)) => JsonAuth::Query {
                name: "key".to_owned(),
                value: key,
            },
            _ => JsonAuth::None,
        };
        let accept = [JsonStaticHeader {
            name: "accept",
            value: "application/json",
        }];
        let static_headers: &[JsonStaticHeader] = if call.accept_json { &accept } else { &[] };
        let response = match call.payload {
            ImagePayload::Json(body) => {
                self.http
                    .post_json(
                        &call.endpoint,
                        &call.path,
                        &[],
                        serde_json::to_vec(&body).map_err(transport_error)?,
                        static_headers,
                        auth,
                        secret_headers,
                        insecure,
                    )
                    .await
            }
            ImagePayload::Multipart(fields) => {
                self.http
                    .post_multipart(
                        &call.endpoint,
                        &call.path,
                        fields,
                        static_headers,
                        auth,
                        secret_headers,
                        insecure,
                    )
                    .await
            }
        }
        .map_err(transport_error)?;
        if !(200..300).contains(&response.status) {
            return Err(api_error(&response));
        }
        if call.binary_response {
            if response.body.is_empty() {
                return Err(ImageProviderError::Failed(
                    "Provider returned an empty image response".to_owned(),
                ));
            }
            return Ok(ProviderImageOutput {
                images: vec![ProviderImage {
                    bytes: response.body,
                    declared_mime_type: None,
                    text: None,
                }],
                usage: None,
            });
        }
        let json = serde_json::from_slice::<Value>(&response.body).map_err(|error| {
            ImageProviderError::Failed(format!("Failed to parse response: {error}"))
        })?;
        let usage = usage::extract_usage(&json);
        let items = adapter.parse(json).map_err(ImageProviderError::Failed)?;
        Ok(ProviderImageOutput {
            images: collect_images(&self.http, items).await?,
            usage,
        })
    }
}
