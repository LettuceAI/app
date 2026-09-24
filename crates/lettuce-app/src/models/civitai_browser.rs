//! Browsing CivitAI for LoRAs with the saved API token, and downloading one
//! into the LoRA library with its trained words and base model.

use std::path::Path;

use lettuce_image_generation::sd_runtime::lora_library::{
    LoraArchitectureSource, LoraKeywordSource, LoraLibraryRepository, normalize_lora_architecture,
    normalize_lora_keywords,
};
use lettuce_image_generation::{
    CIVITAI_API_ENDPOINT, CIVITAI_MAX_PAGE_FETCHES, CivitaiLoraDownload, CivitaiLoraSummary,
    CivitaiModelDetail, CivitaiSearch,
};
use lettuce_model_hub::PinnedArtifact;
use lettuce_network::{
    ArtifactDownloadClient, BulkHttpClient, JsonAuth, JsonQueryParameter, JsonResponse,
    JsonStaticHeader,
};
use lettuce_settings::{
    SecretPurpose, SecretRecord, SecretRef, SecretState, SecretStore, SecretStoreError, SecretValue,
};
use lettuce_types::TimestampMillis;

use crate::{ArtifactInstallPlan, ArtifactSource, PlannedArtifact};

/// Whether a saved CivitAI token works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivitaiAuthStatus {
    pub saved: bool,
    pub valid: bool,
    pub error_kind: Option<CivitaiAuthErrorKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CivitaiAuthErrorKind {
    MissingToken,
    /// CivitAI answered, but not in a way that proves the token works.
    Unverified,
    InvalidOrExpired,
}

impl CivitaiAuthStatus {
    const fn saved(check: TokenCheck) -> Self {
        match check {
            TokenCheck::Valid => Self {
                saved: true,
                valid: true,
                error_kind: None,
            },
            TokenCheck::Unverified => Self {
                saved: true,
                valid: false,
                error_kind: Some(CivitaiAuthErrorKind::Unverified),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenCheck {
    Valid,
    Unverified,
}

/// One search page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CivitaiSearchPage {
    pub items: Vec<CivitaiLoraSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CivitaiBrowser {
    client: BulkHttpClient,
    endpoint: String,
}

fn token_reference() -> (SecretRef, SecretPurpose) {
    let purpose = SecretPurpose::CivitaiAccessToken;
    let reference = purpose
        .app_secret_ref()
        .expect("the CivitAI token has a fixed reference");
    (reference, purpose)
}

async fn present_generation<S: SecretStore + ?Sized>(secrets: &S) -> Result<Option<u64>, String> {
    let (reference, purpose) = token_reference();
    let status = secrets
        .status(&reference, &purpose)
        .await
        .map_err(|error| format!("The CivitAI token could not be read: {error}"))?;
    match status.state {
        SecretState::Present => Ok(Some(status.generation)),
        SecretState::Missing => Ok(None),
        SecretState::Unavailable { reason } => Err(format!(
            "The CivitAI token could not be read: {}",
            SecretStoreError::Unavailable(reason)
        )),
    }
}

impl CivitaiBrowser {
    #[must_use]
    pub fn new(client: BulkHttpClient) -> Self {
        Self::with_endpoint(client, CIVITAI_API_ENDPOINT)
    }

    #[must_use]
    pub fn with_endpoint(client: BulkHttpClient, endpoint: impl Into<String>) -> Self {
        Self {
            client,
            endpoint: endpoint.into(),
        }
    }

    /// The saved token, trimmed; `None` when there is none.
    pub async fn saved_token<S: SecretStore + ?Sized>(
        secrets: &S,
    ) -> Result<Option<SecretValue>, String> {
        let (reference, purpose) = token_reference();
        match secrets.load(&reference, &purpose).await {
            Ok(value) => value.with(|token| {
                let token = token.trim();
                if token.is_empty() {
                    Ok(None)
                } else {
                    SecretValue::new(token)
                        .map(Some)
                        .map_err(|_| "The saved CivitAI token is malformed.".to_owned())
                }
            }),
            Err(SecretStoreError::Missing) => Ok(None),
            Err(error) => Err(format!("The CivitAI token could not be read: {error}")),
        }
    }

    async fn get(
        &self,
        path: &str,
        query: &[(String, String)],
        token: Option<&SecretValue>,
    ) -> Result<JsonResponse, String> {
        let query = query
            .iter()
            .map(|(name, value)| JsonQueryParameter { name, value })
            .collect::<Vec<_>>();
        let auth = token.map_or(JsonAuth::None, |token| {
            token
                .with(|token| SecretValue::new(token))
                .map_or(JsonAuth::None, JsonAuth::Bearer)
        });
        self.client
            .get(
                &self.endpoint,
                path,
                &query,
                &[JsonStaticHeader {
                    name: "user-agent",
                    value: "LettuceAI/1.0",
                }],
                auth,
                Vec::new(),
                false,
            )
            .await
            .map_err(|error| format!("Could not reach CivitAI: {error}"))
    }

    fn failure(response: &JsonResponse, token_saved: bool) -> String {
        lettuce_image_generation::civitai_status_error(
            response.status,
            &lettuce_network::status_text(response.status),
            token_saved,
        )
    }

    /// A page of LoRAs for supported base models, fetching further pages
    /// until the page is full or CivitAI has no more.
    pub async fn search<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        search: &CivitaiSearch,
        pure_active: bool,
    ) -> Result<CivitaiSearchPage, String> {
        let token = Self::saved_token(secrets).await?;
        let target = search.target();
        let mut cursor = search.first_cursor();
        let mut items = Vec::new();
        let mut next_cursor = None;
        for _ in 0..CIVITAI_MAX_PAGE_FETCHES {
            let response = self
                .get(
                    "/api/v1/models",
                    &search.query_parameters(pure_active, cursor.as_deref()),
                    token.as_ref(),
                )
                .await?;
            if !(200..300).contains(&response.status) {
                return Err(Self::failure(&response, token.is_some()));
            }
            let (page, next) =
                lettuce_image_generation::parse_search_page(&response.body, pure_active)?;
            items.extend(page);
            next_cursor = next;
            if next_cursor.is_none() || items.len() >= target {
                break;
            }
            cursor.clone_from(&next_cursor);
        }
        Ok(CivitaiSearchPage { items, next_cursor })
    }

    /// A model with the versions that target a supported base model.
    pub async fn model<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        model_id: u64,
        pure_active: bool,
    ) -> Result<CivitaiModelDetail, String> {
        let token = Self::saved_token(secrets).await?;
        let response = self
            .get(&format!("/api/v1/models/{model_id}"), &[], token.as_ref())
            .await?;
        if response.status == 404 {
            return Err("This CivitAI model no longer exists.".to_owned());
        }
        if !(200..300).contains(&response.status) {
            return Err(Self::failure(&response, token.is_some()));
        }
        lettuce_image_generation::parse_model_detail(&response.body, pure_active)
    }

    async fn validate(&self, token: &SecretValue) -> Result<TokenCheck, String> {
        let response = self
            .client
            .get(
                &self.endpoint,
                "/api/v1/models",
                &[
                    JsonQueryParameter {
                        name: "limit",
                        value: "1",
                    },
                    JsonQueryParameter {
                        name: "hidden",
                        value: "true",
                    },
                ],
                &[JsonStaticHeader {
                    name: "user-agent",
                    value: "LettuceAI/1.0",
                }],
                token
                    .with(|token| SecretValue::new(token))
                    .map_or(JsonAuth::None, JsonAuth::Bearer),
                Vec::new(),
                false,
            )
            .await
            .map_err(|error| format!("Could not validate the CivitAI token: {error}"))?;
        if response.status == 401 {
            return Err("The CivitAI token is invalid or expired.".to_owned());
        }
        Ok(if (200..300).contains(&response.status) {
            TokenCheck::Valid
        } else {
            TokenCheck::Unverified
        })
    }

    pub async fn auth_status<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
    ) -> Result<CivitaiAuthStatus, String> {
        let Some(token) = Self::saved_token(secrets).await? else {
            return Ok(CivitaiAuthStatus {
                saved: false,
                valid: false,
                error_kind: Some(CivitaiAuthErrorKind::MissingToken),
            });
        };
        Ok(match self.validate(&token).await {
            Ok(check) => CivitaiAuthStatus::saved(check),
            Err(_) => CivitaiAuthStatus {
                saved: true,
                valid: false,
                error_kind: Some(CivitaiAuthErrorKind::InvalidOrExpired),
            },
        })
    }

    /// Saves the token unless CivitAI refuses it.
    pub async fn save_token<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        token: &str,
    ) -> Result<CivitaiAuthStatus, String> {
        let token = token.trim();
        if token.is_empty() {
            return Err("Enter a CivitAI API token.".to_owned());
        }
        let value =
            SecretValue::new(token).map_err(|_| "The CivitAI token is malformed.".to_owned())?;
        let expected = present_generation(secrets).await?;
        let check = self.validate(&value).await?;
        let (reference, purpose) = token_reference();
        secrets
            .put(SecretRecord::new(reference, purpose), value, expected)
            .await
            .map_err(|error| format!("The CivitAI token could not be saved: {error}"))?;
        Ok(CivitaiAuthStatus::saved(check))
    }

    pub async fn clear_token<S: SecretStore + ?Sized>(secrets: &S) -> Result<(), String> {
        let Some(generation) = present_generation(secrets).await? else {
            return Ok(());
        };
        let (reference, purpose) = token_reference();
        match secrets.delete(&reference, &purpose, Some(generation)).await {
            Ok(_) | Err(SecretStoreError::Missing) => Ok(()),
            Err(error) => Err(format!("The CivitAI token could not be removed: {error}")),
        }
    }

    /// A download client signed in with the saved token.
    pub async fn download_client<S: SecretStore + ?Sized>(
        secrets: &S,
    ) -> Result<ArtifactDownloadClient, String> {
        let client = ArtifactDownloadClient::new()
            .map_err(|error| format!("Failed to build download client: {error}"))?;
        Ok(match Self::saved_token(secrets).await? {
            Some(token) => client.with_civitai_token(token),
            None => client,
        })
    }
}

/// The install that puts a CivitAI LoRA into the library, sized by asking
/// CivitAI for it and checked against its SHA-256 when CivitAI lists one.
pub async fn civitai_lora_install_plan(
    client: &ArtifactDownloadClient,
    lora_root: &Path,
    download: &CivitaiLoraDownload,
    token_saved: bool,
) -> Result<ArtifactInstallPlan, String> {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_owned());
    }
    let (filename, url) = download.target()?;
    let size = client
        .probe_https_size(&url)
        .await
        .map_err(|error| match error {
            lettuce_network::ArtifactProbeError::Refused(401) if token_saved => {
                "The saved CivitAI token is invalid or expired.".to_owned()
            }
            lettuce_network::ArtifactProbeError::Refused(401) => {
                "This CivitAI file requires an API token. Add one in Runtime Defaults.".to_owned()
            }
            lettuce_network::ArtifactProbeError::Refused(403) => {
                "This CivitAI file is restricted or in early access for your account.".to_owned()
            }
            lettuce_network::ArtifactProbeError::Refused(status) => format!(
                "Download failed with status: {}",
                lettuce_network::status_text(status)
            ),
            lettuce_network::ArtifactProbeError::Download(error) => {
                format!("Failed to start download: {error}")
            }
        })?;
    let source = ArtifactSource::Https { url };
    Ok(ArtifactInstallPlan {
        install_id: format!("civitai-lora:{}:{filename}", download.version_id),
        root: lora_root.to_path_buf(),
        artifacts: vec![PlannedArtifact {
            artifact: PinnedArtifact {
                source_identity: source.identity(),
                local_segments: vec![filename],
                byte_size: size,
                sha256: download.normalized_sha256(),
            },
            source,
        }],
    })
}

/// Records a downloaded CivitAI LoRA with the trained words and base model
/// CivitAI lists for it.
pub fn record_civitai_lora<R: LoraLibraryRepository + ?Sized>(
    repository: &R,
    lora_root: &Path,
    download: &CivitaiLoraDownload,
    now: TimestampMillis,
) -> Result<(), String> {
    let (filename, _) = download.target()?;
    let (bytes_on_disk, modified_at) =
        lettuce_image_generation::sd_runtime::loras::lora_file_fingerprint(
            &lora_root.join(&filename),
        )?;
    let mut record = repository
        .record_lora_file(&filename, &filename, bytes_on_disk, modified_at, now)
        .map_err(|error| error.to_string())?;
    let keywords = normalize_lora_keywords(download.trained_words.clone());
    let architecture = download
        .base_model
        .as_deref()
        .and_then(normalize_lora_architecture);
    record.sha256 = download.normalized_sha256();
    record.keyword_source = if keywords.is_empty() {
        LoraKeywordSource::None
    } else {
        LoraKeywordSource::Civitai
    };
    record.architecture_source = if architecture.is_some() {
        LoraArchitectureSource::Civitai
    } else {
        LoraArchitectureSource::None
    };
    record.keywords = keywords;
    record.architecture = architecture;
    repository
        .save_lora(&record, now)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository;
    use lettuce_settings::InMemorySecretStore;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    async fn server(
        responses: Vec<(u16, String)>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let handle = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.expect("accept");
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).await.expect("read");
                requests.push(String::from_utf8_lossy(&buffer[..read]).into_owned());
                let response = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.expect("write");
            }
            requests
        });
        (format!("http://{address}"), handle)
    }

    fn page(id: u64, cursor: Option<&str>) -> String {
        let metadata =
            cursor.map_or_else(String::new, |cursor| format!(r#""nextCursor": "{cursor}""#));
        format!(
            r#"{{"items": [{{"id": {id}, "name": "L{id}", "type": "LORA", "modelVersions": [{{"id": {id}0, "baseModel": "ZImageTurbo"}}]}}], "metadata": {{{metadata}}}}}"#
        )
    }

    #[tokio::test]
    async fn a_search_gathers_pages_until_it_is_full_or_civitai_runs_out() {
        let (endpoint, requests) = server(vec![
            (200, page(1, Some("c2"))),
            (200, page(2, None)),
            (404, "{}".to_owned()),
            (401, "{}".to_owned()),
            (200, "{}".to_owned()),
        ])
        .await;
        let browser =
            CivitaiBrowser::with_endpoint(BulkHttpClient::new().expect("client"), endpoint);
        let secrets = InMemorySecretStore::default();
        let found = browser
            .search(
                &secrets,
                &CivitaiSearch {
                    limit: Some(5),
                    ..CivitaiSearch::default()
                },
                true,
            )
            .await
            .expect("search");
        assert_eq!(
            found.items.iter().map(|item| item.id).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(found.next_cursor, None);
        assert_eq!(
            browser.model(&secrets, 9, false).await,
            Err("This CivitAI model no longer exists.".to_owned())
        );
        assert_eq!(
            browser.save_token(&secrets, " bad ").await,
            Err("The CivitAI token is invalid or expired.".to_owned())
        );
        assert_eq!(
            browser.save_token(&secrets, "good").await,
            Ok(CivitaiAuthStatus {
                saved: true,
                valid: true,
                error_kind: None
            })
        );
        let requests = requests.await.expect("requests");
        assert!(requests[0].contains("nsfw=false"));
        assert!(requests[1].contains("cursor=c2"));
        assert!(requests[4].starts_with("GET /api/v1/models?limit=1&hidden=true "));
        assert!(
            requests[4]
                .to_lowercase()
                .contains("authorization: bearer good")
        );
        CivitaiBrowser::clear_token(&secrets).await.expect("clear");
        assert!(matches!(
            CivitaiBrowser::saved_token(&secrets).await,
            Ok(None)
        ));
    }

    #[test]
    fn a_downloaded_lora_keeps_what_civitai_says_about_it() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let root = std::env::temp_dir().join(format!(
            "civitai-lora-{}",
            lettuce_types::OperationId::new()
        ));
        std::fs::create_dir_all(&root).expect("root");
        std::fs::write(root.join("style.safetensors"), b"lora").expect("file");
        let download = CivitaiLoraDownload {
            model_name: "Style".to_owned(),
            version_id: 4,
            file_name: "style.safetensors".to_owned(),
            sha256: Some(format!(" {} ", "AB".repeat(32))),
            trained_words: vec!["ink".to_owned(), "INK".to_owned()],
            base_model: Some("ZImageTurbo".to_owned()),
            ..CivitaiLoraDownload::default()
        };
        record_civitai_lora(
            backend.database(),
            &root,
            &download,
            TimestampMillis::new(2),
        )
        .expect("record");
        let record = backend
            .database()
            .lora("style.safetensors")
            .expect("lora")
            .expect("stored");
        assert_eq!(record.keywords, ["ink"]);
        assert_eq!(record.keyword_source, LoraKeywordSource::Civitai);
        assert_eq!(record.architecture.as_deref(), Some("z-image"));
        assert_eq!(record.sha256, Some("ab".repeat(32)));
        let _ = std::fs::remove_dir_all(root);
    }
}
