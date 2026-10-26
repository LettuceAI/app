//! Reading GGUF headers: the leading bytes of a Hugging Face file, a
//! downloaded file's shape, and the Sprout probe of a remote Ollama machine.

use std::io::Read;
use std::path::Path;

use async_trait::async_trait;
use lettuce_model_hub::{GGUF_HEADER_PROBE_BYTES, GGUF_HEADER_RETRY_BYTES, GgufModelMeta};
use lettuce_network::ArtifactDownloadClient;
use lettuce_settings::{SecretStore, SecretValue};

/// The leading bytes of a file in a Hugging Face repository.
#[async_trait]
pub trait GgufHeaderSource: Send + Sync {
    async fn read_prefix(
        &self,
        model_id: &str,
        revision: &str,
        filename: &str,
        length: u64,
        token: Option<&SecretValue>,
    ) -> Option<Vec<u8>>;
    async fn read_range(
        &self,
        model_id: &str,
        revision: &str,
        filename: &str,
        range: (u64, u64),
        expected_size: u64,
        token: Option<&SecretValue>,
    ) -> Option<Vec<u8>> {
        let _ = expected_size;
        let (offset, length) = range;
        let bytes = self
            .read_prefix(
                model_id,
                revision,
                filename,
                offset.checked_add(length)?,
                token,
            )
            .await?;
        Some(bytes.get(usize::try_from(offset).ok()?..)?.to_vec())
    }
}

#[async_trait]
impl GgufHeaderSource for ArtifactDownloadClient {
    async fn read_prefix(
        &self,
        model_id: &str,
        revision: &str,
        filename: &str,
        length: u64,
        token: Option<&SecretValue>,
    ) -> Option<Vec<u8>> {
        let url = lettuce_model_hub::resolve_url(model_id, revision, filename).ok()?;
        self.read_hugging_face_prefix(
            &url,
            lettuce_model_hub::HUGGING_FACE_ENDPOINT,
            length,
            token,
        )
        .await
        .ok()
    }
    async fn read_range(
        &self,
        model_id: &str,
        revision: &str,
        filename: &str,
        range: (u64, u64),
        expected_size: u64,
        token: Option<&SecretValue>,
    ) -> Option<Vec<u8>> {
        let url = lettuce_model_hub::resolve_url(model_id, revision, filename).ok()?;
        self.read_hugging_face_range(
            &url,
            lettuce_model_hub::HUGGING_FACE_ENDPOINT,
            range,
            Some(expected_size),
            token,
        )
        .await
        .ok()
    }
}

/// The `/specs` report of an Ollama account's Sprout probe; `None` when the
/// account has no active probe.
pub(crate) async fn sprout_specs_bytes<S: SecretStore + ?Sized>(
    client: &lettuce_network::JsonClient,
    secrets: &S,
    account: &lettuce_models::ProviderAccount,
) -> Result<Option<Vec<u8>>, String> {
    let Some(sprout) = account.config.active_sprout() else {
        return Ok(None);
    };
    let base = sprout.url.trim().trim_end_matches('/');
    let endpoint = lettuce_model_hub::sprout_specs_url(base);
    let auth = match sprout.api_key_ref {
        Some(reference) => {
            let purpose = lettuce_settings::SecretPurpose::SproutApiKey {
                owner: account.secret_owner_id,
            };
            match secrets.load(&reference, &purpose).await {
                Ok(key) if key.with(|key| !key.trim().is_empty()) => {
                    lettuce_network::JsonAuth::Bearer(key)
                }
                Ok(_) | Err(lettuce_settings::SecretStoreError::Missing) => {
                    lettuce_network::JsonAuth::None
                }
                Err(error) => {
                    return Err(format!("The Sprout API key could not be read: {error}"));
                }
            }
        }
        None => lettuce_network::JsonAuth::None,
    };
    let response = client
        .get_json(
            base,
            "/specs",
            &[lettuce_network::JsonStaticHeader {
                name: "user-agent",
                value: "LettuceAI/1.0",
            }],
            auth,
            Vec::new(),
            lettuce_network::RequestPolicy::BROWSE,
        )
        .await
        .map_err(|error| format!("Failed to reach Sprout at {endpoint}: {error}"))?;
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "Sprout at {endpoint} returned {}",
            lettuce_network::status_text(response.status)
        ));
    }
    Ok(Some(response.body))
}

/// A repository at a revision: the commit the model page pinned, else
/// `main`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteModel<'a> {
    pub id: &'a str,
    pub revision: &'a str,
}

fn read_prefix(path: &Path, length: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(length)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

pub(crate) fn local_gguf_meta(path: &Path) -> Option<GgufModelMeta> {
    let probe = read_prefix(path, GGUF_HEADER_PROBE_BYTES)?;
    lettuce_model_hub::gguf_meta_with_retry(&probe, || read_prefix(path, GGUF_HEADER_RETRY_BYTES))
}

#[cfg(test)]
mod tests {
    use lettuce_settings::InMemorySecretStore;

    use super::*;

    #[tokio::test]
    async fn an_ollama_account_reads_its_specs_from_sprout() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).await.expect("read");
            let body = r#"{"schemaVersion": 1, "availableMemoryBytes": 64, "gpus": [{"memoryFree": 24, "deviceType": "Gpu"}]}"#;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .expect("write");
            String::from_utf8_lossy(&buffer[..read]).into_owned()
        });
        let secrets = InMemorySecretStore::default();
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = lettuce_settings::SecretRef::new();
        lettuce_settings::SecretStore::put(
            &secrets,
            lettuce_settings::SecretRecord::new(
                key_ref,
                lettuce_settings::SecretPurpose::SproutApiKey { owner },
            ),
            SecretValue::new("sprout-key").expect("key"),
            None,
        )
        .await
        .expect("put");
        let mut account = lettuce_models::ProviderAccount {
            id: lettuce_types::ProviderAccountId::new(),
            secret_owner_id: owner,
            provider_kind: "ollama".to_owned(),
            protocol: lettuce_models::ProviderProtocol::Ollama,
            label: "Remote".to_owned(),
            endpoint: None,
            enabled: true,
            streaming_enabled: true,
            allow_invalid_tls: false,
            api_key_ref: None,
            secret_headers: Vec::new(),
            config: lettuce_models::ProviderConfig::Ollama(lettuce_models::OllamaConfig {
                sprout: Some(lettuce_models::SproutConfig {
                    enabled: true,
                    url: format!("http://{address}/"),
                    api_key_ref: Some(key_ref),
                }),
            }),
            revision: lettuce_types::Revision::INITIAL,
            created_at: lettuce_types::TimestampMillis::new(1),
            updated_at: lettuce_types::TimestampMillis::new(1),
        };
        let client = lettuce_network::JsonClient::new().expect("client");
        let specs = sprout_specs_bytes(&client, &secrets, &account)
            .await
            .expect("specs")
            .expect("active");
        let specs = lettuce_model_hub::sprout_preview_specs("Sprout", &specs).expect("report");
        assert_eq!(specs.available_memory_bytes, 64);
        assert_eq!(specs.gpus[0].memory_free, 24);
        let request = server.await.expect("request");
        assert!(request.starts_with("GET /specs HTTP/1.1"));
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer sprout-key")
        );
        account.config = lettuce_models::ProviderConfig::Standard;
        assert_eq!(
            sprout_specs_bytes(&client, &secrets, &account).await,
            Ok(None)
        );
    }
}
