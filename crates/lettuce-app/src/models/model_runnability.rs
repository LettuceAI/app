//! How well a model runs here: scores and recommendations for Hugging Face
//! files from their GGUF header, and the score of a downloaded model file.

use std::io::Read;
use std::path::Path;

use async_trait::async_trait;
use lettuce_model_hub::{
    GGUF_HEADER_PROBE_BYTES, GGUF_HEADER_RETRY_BYTES, GgufModelMeta, HfBrowseError,
    LocalRunnability, RecommendationData, RunnabilityDefaults, RunnabilityFile,
    RunnabilityHardware, RunnabilityScore,
};
use lettuce_network::ArtifactDownloadClient;
use lettuce_settings::{SecretStore, SecretValue};

use crate::HuggingFaceBrowser;

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
}

/// This machine's memory for llama.cpp; nothing on mobile, where models do
/// not run locally.
#[must_use]
pub fn local_runnability_hardware() -> RunnabilityHardware {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let supports_gpu_offload = lettuce_local_llm::llama::shared_backend()
            .is_ok_and(|backend| backend.supports_gpu_offload());
        RunnabilityHardware {
            available_ram: lettuce_local_llm::hardware::get_available_memory_bytes(),
            available_vram: if supports_gpu_offload {
                lettuce_local_llm::hardware::get_available_vram_bytes()
            } else {
                None
            },
            supports_gpu_offload,
            unified_memory: lettuce_local_llm::hardware::is_unified_memory(),
        }
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        RunnabilityHardware {
            available_ram: None,
            available_vram: None,
            supports_gpu_offload: false,
            unified_memory: false,
        }
    }
}

/// The hardware models downloaded for `destination` would run on: this
/// machine for a local download, the remote machine behind an Ollama
/// account's active Sprout probe; `None` for an Ollama account without one,
/// whose hardware the app cannot see.
pub async fn runnability_hardware<S: SecretStore + ?Sized>(
    client: &lettuce_network::JsonClient,
    secrets: &S,
    destination: Option<&lettuce_models::ProviderAccount>,
) -> Result<Option<RunnabilityHardware>, String> {
    match destination {
        None => Ok(Some(local_runnability_hardware())),
        Some(account) => sprout_runnability_hardware(client, secrets, account).await,
    }
}

/// The hardware behind an Ollama account's Sprout probe; `None` when the
/// account has no active probe.
pub async fn sprout_runnability_hardware<S: SecretStore + ?Sized>(
    client: &lettuce_network::JsonClient,
    secrets: &S,
    account: &lettuce_models::ProviderAccount,
) -> Result<Option<RunnabilityHardware>, String> {
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
    lettuce_model_hub::sprout_hardware(&endpoint, &response.body).map(Some)
}

/// A repository at a revision: the commit the model page pinned, else
/// `main`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteModel<'a> {
    pub id: &'a str,
    pub revision: &'a str,
}

impl HuggingFaceBrowser {
    async fn remote_gguf_meta<S, H>(
        &self,
        secrets: &S,
        source: &H,
        model: RemoteModel<'_>,
        files: &[RunnabilityFile],
    ) -> Result<Option<GgufModelMeta>, HfBrowseError>
    where
        S: SecretStore + ?Sized,
        H: GgufHeaderSource + ?Sized,
    {
        let Some(representative) = files
            .iter()
            .filter(|file| file.size > 0)
            .min_by_key(|file| file.size)
        else {
            return Ok(None);
        };
        let key = (
            model.id.to_owned(),
            model.revision.to_owned(),
            representative.filename.clone(),
        );
        if let Some(cached) = self
            .headers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
        {
            return Ok(Some(cached.clone()));
        }
        let meta = self
            .read_remote_gguf_meta(secrets, source, model, representative)
            .await?;
        if let Some(meta) = &meta {
            self.headers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(key, meta.clone());
        }
        Ok(meta)
    }

    /// The header of `representative`, read within the browse budget as a
    /// whole (both reads together); `None` when it could not be read in
    /// time.
    async fn read_remote_gguf_meta<S, H>(
        &self,
        secrets: &S,
        source: &H,
        model: RemoteModel<'_>,
        representative: &RunnabilityFile,
    ) -> Result<Option<GgufModelMeta>, HfBrowseError>
    where
        S: SecretStore + ?Sized,
        H: GgufHeaderSource + ?Sized,
    {
        let token = Self::saved_token(secrets).await?;
        let read = |length| {
            source.read_prefix(
                model.id,
                model.revision,
                &representative.filename,
                length,
                token.as_ref(),
            )
        };
        let reading = async {
            let probe = read(GGUF_HEADER_PROBE_BYTES).await?;
            let first = lettuce_model_hub::parse_gguf_meta(&probe);
            if first.as_ref().is_none_or(GgufModelMeta::has_essentials) {
                return first;
            }
            read(GGUF_HEADER_RETRY_BYTES)
                .await
                .and_then(|data| lettuce_model_hub::parse_gguf_meta(&data))
                .or(first)
        };
        Ok(
            tokio::time::timeout(lettuce_network::BROWSE_TIMEOUT, reading)
                .await
                .ok()
                .flatten(),
        )
    }

    /// A score per file, estimated from the smallest file's GGUF header,
    /// and whether that header could be read.
    pub async fn runnability<S, H>(
        &self,
        secrets: &S,
        source: &H,
        model: RemoteModel<'_>,
        files: &[RunnabilityFile],
        hardware: RunnabilityHardware,
        defaults: RunnabilityDefaults,
    ) -> Result<(Vec<RunnabilityScore>, bool), HfBrowseError>
    where
        S: SecretStore + ?Sized,
        H: GgufHeaderSource + ?Sized,
    {
        if files.is_empty() {
            return Ok((Vec::new(), false));
        }
        let meta = self.remote_gguf_meta(secrets, source, model, files).await?;
        Ok((
            lettuce_model_hub::runnability_scores(
                files,
                model.id,
                meta.as_ref(),
                hardware,
                defaults,
            ),
            meta.is_some(),
        ))
    }

    /// Context limits per file and the recommended file, context and KV
    /// type, and whether the GGUF header could be read.
    pub async fn recommendation<S, H>(
        &self,
        secrets: &S,
        source: &H,
        model: RemoteModel<'_>,
        files: &[RunnabilityFile],
        hardware: RunnabilityHardware,
        defaults: RunnabilityDefaults,
    ) -> Result<(RecommendationData, bool), HfBrowseError>
    where
        S: SecretStore + ?Sized,
        H: GgufHeaderSource + ?Sized,
    {
        if files.is_empty() {
            return Ok((RecommendationData::empty(), false));
        }
        let meta = self.remote_gguf_meta(secrets, source, model, files).await?;
        Ok((
            lettuce_model_hub::build_recommendation(
                files,
                model.id,
                meta.as_ref(),
                hardware,
                defaults.context_length,
            ),
            meta.is_some(),
        ))
    }
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

/// Files loaded next to a model: its projector, and its draft model (the
/// DFlash drafter when DFlash is enabled, else the MTP one) unless the draft
/// placement is the CPU.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalModelSidecars<'a> {
    pub mmproj_path: Option<&'a str>,
    pub mtp_enabled: bool,
    pub mtp_on_cpu: bool,
    pub mtp_model_path: Option<&'a str>,
    pub dflash_enabled: bool,
    pub dflash_model_path: Option<&'a str>,
}

fn file_size(path: Option<&str>) -> u64 {
    path.filter(|path| !path.trim().is_empty())
        .and_then(|path| std::fs::metadata(path).ok())
        .map_or(0, |metadata| metadata.len())
}

fn sidecar_bytes(sidecars: &LocalModelSidecars<'_>) -> u64 {
    let drafter = if sidecars.mtp_on_cpu {
        0
    } else {
        file_size(lettuce_local_llm::offload::drafter_reserve_path(
            sidecars.dflash_enabled,
            sidecars.dflash_model_path,
            sidecars.mtp_enabled,
            sidecars.mtp_model_path,
        ))
    };
    file_size(sidecars.mmproj_path).saturating_add(drafter)
}

/// The score of a downloaded GGUF file on `hardware`, and whether its GGUF
/// header could be read.
pub fn local_file_runnability(
    file_path: &str,
    sidecars: &LocalModelSidecars<'_>,
    hardware: RunnabilityHardware,
    defaults: RunnabilityDefaults,
) -> Result<(LocalRunnability, bool), String> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err("File does not exist".to_owned());
    }
    let size = std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|error| format!("Failed to read file metadata: {error}"))?;
    let sidecar_bytes = sidecar_bytes(sidecars);
    let meta = local_gguf_meta(path);
    Ok((
        lettuce_model_hub::local_runnability(
            file_path,
            size,
            meta.as_ref(),
            hardware,
            sidecar_bytes,
            defaults,
        ),
        meta.is_some(),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use lettuce_settings::InMemorySecretStore;

    use super::*;

    const MAIN: RemoteModel<'static> = RemoteModel {
        id: "org/m",
        revision: "main",
    };

    struct Header {
        bytes: Vec<u8>,
        reads: Mutex<Vec<(String, u64)>>,
    }

    #[async_trait]
    impl GgufHeaderSource for Header {
        async fn read_prefix(
            &self,
            model_id: &str,
            _revision: &str,
            filename: &str,
            length: u64,
            _token: Option<&SecretValue>,
        ) -> Option<Vec<u8>> {
            self.reads
                .lock()
                .expect("reads")
                .push((format!("{model_id}/{filename}"), length));
            let end = usize::try_from(length)
                .expect("length")
                .min(self.bytes.len());
            Some(self.bytes[..end].to_vec())
        }
    }

    struct Stalled;

    #[async_trait]
    impl GgufHeaderSource for Stalled {
        async fn read_prefix(
            &self,
            _model_id: &str,
            _revision: &str,
            _filename: &str,
            _length: u64,
            _token: Option<&SecretValue>,
        ) -> Option<Vec<u8>> {
            std::future::pending().await
        }
    }

    struct Revisions(Mutex<Vec<String>>);

    #[async_trait]
    impl GgufHeaderSource for Revisions {
        async fn read_prefix(
            &self,
            _model_id: &str,
            revision: &str,
            _filename: &str,
            length: u64,
            _token: Option<&SecretValue>,
        ) -> Option<Vec<u8>> {
            self.0.lock().expect("revisions").push(revision.to_owned());
            let bytes = header();
            let end = usize::try_from(length).expect("length").min(bytes.len());
            Some(bytes[..end].to_vec())
        }
    }

    fn hardware() -> RunnabilityHardware {
        RunnabilityHardware {
            available_ram: Some(32_000_000_000),
            available_vram: Some(12_000_000_000),
            supports_gpu_offload: true,
            unified_memory: false,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_header_that_never_arrives_gives_up_after_the_browse_budget() {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let files = [file("m-Q4_K_M.gguf", 4_000_000_000)];
        let started = tokio::time::Instant::now();
        let (scores, metadata) = browser
            .runnability(
                &InMemorySecretStore::default(),
                &Stalled,
                MAIN,
                &files,
                hardware(),
                RunnabilityDefaults::new(None, None),
            )
            .await
            .expect("scores without a header");
        assert!(!metadata);
        assert_eq!(scores.len(), 1);
        assert_eq!(started.elapsed(), lettuce_network::BROWSE_TIMEOUT);
    }

    #[tokio::test]
    async fn headers_are_cached_per_revision() {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let source = Revisions(Mutex::new(Vec::new()));
        let files = [file("m-Q4_K_M.gguf", 4_000_000_000)];
        for revision in ["aaa", "aaa", "bbb"] {
            browser
                .runnability(
                    &InMemorySecretStore::default(),
                    &source,
                    RemoteModel {
                        id: "org/m",
                        revision,
                    },
                    &files,
                    hardware(),
                    RunnabilityDefaults::new(None, None),
                )
                .await
                .expect("scores");
        }
        assert_eq!(*source.0.lock().expect("revisions"), ["aaa", "bbb"]);
    }

    fn header() -> Vec<u8> {
        let mut out = b"GGUF".to_vec();
        out.extend(3_u32.to_le_bytes());
        out.extend(0_u64.to_le_bytes());
        let entries: [(&str, u32); 5] = [
            ("llama.block_count", 32),
            ("llama.embedding_length", 4096),
            ("llama.attention.head_count", 32),
            ("llama.attention.head_count_kv", 8),
            ("llama.context_length", 8192),
        ];
        out.extend((entries.len() as u64 + 1).to_le_bytes());
        let architecture = "general.architecture";
        out.extend((architecture.len() as u64).to_le_bytes());
        out.extend(architecture.as_bytes());
        out.extend(8_u32.to_le_bytes());
        out.extend(5_u64.to_le_bytes());
        out.extend(b"llama");
        for (key, value) in entries {
            out.extend((key.len() as u64).to_le_bytes());
            out.extend(key.as_bytes());
            out.extend(4_u32.to_le_bytes());
            out.extend(value.to_le_bytes());
        }
        out
    }

    fn file(filename: &str, size: u64) -> RunnabilityFile {
        RunnabilityFile {
            filename: filename.to_owned(),
            size,
            quantization: lettuce_model_hub::extract_quantization(filename),
        }
    }

    #[tokio::test]
    async fn scores_come_from_the_smallest_files_header() {
        let source = Header {
            bytes: header(),
            reads: Mutex::new(Vec::new()),
        };
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let secrets = InMemorySecretStore::default();
        let hardware = RunnabilityHardware {
            available_ram: Some(32_000_000_000),
            available_vram: Some(12_000_000_000),
            supports_gpu_offload: true,
            unified_memory: false,
        };
        let defaults = RunnabilityDefaults::new(None, None);
        let files = [
            file("m-Q8_0.gguf", 8_000_000_000),
            file("m-Q4_K_M.gguf", 4_000_000_000),
            file("m-F16.gguf", 0),
        ];
        let (scores, metadata) = browser
            .runnability(&secrets, &source, MAIN, &files, hardware, defaults)
            .await
            .expect("scores");
        assert!(metadata);
        assert_eq!(scores.len(), 3);
        assert_eq!(
            source.reads.lock().expect("reads").as_slice(),
            [("org/m/m-Q4_K_M.gguf".to_owned(), GGUF_HEADER_PROBE_BYTES)]
        );
        let (recommendation, _) = browser
            .recommendation(&secrets, &source, MAIN, &files, hardware, defaults)
            .await
            .expect("recommendation");
        assert_eq!(recommendation.model_max_context, 8192);
        assert_eq!(recommendation.files.len(), 2);
        assert!(recommendation.best.is_some());
        assert_eq!(
            source.reads.lock().expect("reads").len(),
            1,
            "a header read once is reused"
        );
        let truncated = Header {
            bytes: header()[..90].to_vec(),
            reads: Mutex::new(Vec::new()),
        };
        let fresh = || HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let (recommendation, _) = fresh()
            .recommendation(&secrets, &truncated, MAIN, &files, hardware, defaults)
            .await
            .expect("recommendation");
        assert!(recommendation.arch.expect("arch").incomplete_parse);
        assert_eq!(
            truncated
                .reads
                .lock()
                .expect("reads")
                .iter()
                .map(|(_, length)| *length)
                .collect::<Vec<_>>(),
            [GGUF_HEADER_PROBE_BYTES, GGUF_HEADER_RETRY_BYTES]
        );
        assert_eq!(
            browser
                .recommendation(&secrets, &source, MAIN, &[], hardware, defaults)
                .await
                .expect("empty"),
            (RecommendationData::empty(), false)
        );
        let unreadable = Header {
            bytes: b"not a gguf".to_vec(),
            reads: Mutex::new(Vec::new()),
        };
        let (scores, metadata) = fresh()
            .runnability(&secrets, &unreadable, MAIN, &files, hardware, defaults)
            .await
            .expect("scores without a header");
        assert_eq!(scores.len(), 3);
        assert!(!metadata);
    }

    #[tokio::test]
    async fn an_ollama_account_reads_its_hardware_from_sprout() {
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
        let hardware = sprout_runnability_hardware(&client, &secrets, &account)
            .await
            .expect("hardware")
            .expect("active");
        assert_eq!(hardware.available_ram, Some(64));
        assert_eq!(hardware.available_vram, Some(24));
        let request = server.await.expect("request");
        assert!(request.starts_with("GET /specs HTTP/1.1"));
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer sprout-key")
        );
        account.config = lettuce_models::ProviderConfig::Standard;
        assert_eq!(
            runnability_hardware(&client, &secrets, Some(&account)).await,
            Ok(None)
        );
    }

    #[test]
    fn a_downloaded_file_counts_its_gpu_sidecars() {
        let dir = std::env::temp_dir().join(format!("lettuce-runnability-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let model = dir.join("model-Q4_K_M.gguf");
        let mmproj = dir.join("mmproj.gguf");
        std::fs::write(&model, header()).expect("model");
        std::fs::write(&mmproj, vec![0_u8; 1000]).expect("mmproj");
        let hardware = RunnabilityHardware {
            available_ram: Some(8_000_000_000),
            available_vram: Some(4_000_000_000),
            supports_gpu_offload: true,
            unified_memory: false,
        };
        let defaults = RunnabilityDefaults::new(None, None);
        let model_path = model.to_string_lossy().into_owned();
        let mmproj_path = mmproj.to_string_lossy().into_owned();
        let (bare, metadata) = local_file_runnability(
            &model_path,
            &LocalModelSidecars::default(),
            hardware,
            defaults,
        )
        .expect("bare");
        assert!(metadata);
        assert_eq!(bare.quantization, "Q4_K_M");
        assert_eq!(bare.model_size, header().len() as u64);
        let (with_projector, _) = local_file_runnability(
            &model_path,
            &LocalModelSidecars {
                mmproj_path: Some(&mmproj_path),
                ..LocalModelSidecars::default()
            },
            RunnabilityHardware {
                supports_gpu_offload: false,
                ..hardware
            },
            defaults,
        )
        .expect("cpu");
        assert_eq!(with_projector.available_vram, 0);
        assert_eq!(
            local_file_runnability(
                &dir.join("missing.gguf").to_string_lossy(),
                &LocalModelSidecars::default(),
                hardware,
                defaults
            )
            .map(|_| ()),
            Err("File does not exist".to_owned())
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn local_runnability_counts_the_dflash_drafter_in_the_mtp_slot() {
        let dir = std::env::temp_dir().join(format!(
            "lettuce-runnability-drafter-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let write = |name: &str, size: usize| {
            let path = dir.join(name);
            std::fs::write(&path, vec![0_u8; size]).expect("file");
            path.to_string_lossy().into_owned()
        };
        let mmproj = write("mmproj.gguf", 10);
        let mtp = write("mtp-m.gguf", 100);
        let dflash = write("m-dflash.gguf", 1000);
        let both = LocalModelSidecars {
            mmproj_path: Some(&mmproj),
            mtp_enabled: true,
            mtp_on_cpu: false,
            mtp_model_path: Some(&mtp),
            dflash_enabled: true,
            dflash_model_path: Some(&dflash),
        };
        assert_eq!(sidecar_bytes(&both), 1010);
        assert_eq!(
            sidecar_bytes(&LocalModelSidecars {
                dflash_model_path: None,
                mtp_enabled: false,
                ..both.clone()
            }),
            110
        );
        assert_eq!(
            sidecar_bytes(&LocalModelSidecars {
                dflash_enabled: false,
                ..both.clone()
            }),
            110
        );
        assert_eq!(
            sidecar_bytes(&LocalModelSidecars {
                dflash_enabled: false,
                mtp_enabled: false,
                ..both.clone()
            }),
            10
        );
        assert_eq!(
            sidecar_bytes(&LocalModelSidecars {
                mtp_on_cpu: true,
                ..both
            }),
            10
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
