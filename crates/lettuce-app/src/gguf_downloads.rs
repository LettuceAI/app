//! GGUF models downloaded from Hugging Face: the folder they go to, the
//! pinned install of a model with its projector and MTP draft model, and the
//! llama.cpp model registered once they are on disk.

use std::path::{Path, PathBuf};

use lettuce_model_hub::{HfBrowseError, HfResource, PinnedArtifact};
use lettuce_models::{
    ModelLookup, ModelProfileRepository, ProviderAccount, ProviderAccountRepository,
    ProviderConfig, ProviderProtocol,
};
use lettuce_settings::{DeviceSettings, SecretOwnerId, SecretStore};
use lettuce_types::{ProviderAccountId, Revision, TimestampMillis};
use serde_json::{Map, Value, json};

use crate::{
    ArtifactInstallPlan, ArtifactSource, HuggingFaceBrowser, ImportedModelFile,
    ModelFileCoordinator, ModelFileError, PlannedArtifact,
};

pub const LOCAL_LLAMA_PROVIDER_KIND: &str = "llamacpp";
pub const LOCAL_LLAMA_PROVIDER_LABEL: &str = "llama.cpp (Local)";

/// Where GGUF downloads go: the device's chosen folder, else
/// `<app folder>/models/gguf`.
#[must_use]
pub fn llm_models_root(device: &DeviceSettings, app_folder: &Path) -> PathBuf {
    device
        .llm_models_dir
        .as_deref()
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| app_folder.join("models").join("gguf"), PathBuf::from)
}

/// The files of one GGUF install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GgufDownload {
    pub model_id: String,
    pub model_file: String,
    pub mmproj_file: Option<String>,
    pub mtp_file: Option<String>,
}

impl GgufDownload {
    fn filenames(&self) -> Vec<&str> {
        std::iter::once(self.model_file.as_str())
            .chain(self.mmproj_file.as_deref())
            .chain(self.mtp_file.as_deref())
            .collect()
    }

    fn local_segments(&self, filename: &str) -> Vec<String> {
        std::iter::once(self.model_id.replace('/', "--"))
            .chain(filename.split('/').map(str::to_owned))
            .collect()
    }

    /// The file name without `.gguf`, else the repository name.
    #[must_use]
    pub fn default_display_name(&self) -> String {
        let base = self
            .model_file
            .rsplit('/')
            .next()
            .unwrap_or(&self.model_file);
        let name = base
            .len()
            .checked_sub(5)
            .and_then(|end| {
                base.get(end..)
                    .filter(|extension| extension.eq_ignore_ascii_case(".gguf"))
                    .and_then(|_| base.get(..end))
            })
            .unwrap_or(base);
        if !name.is_empty() {
            return name.to_owned();
        }
        match self.model_id.rsplit('/').next() {
            Some(short) if !short.is_empty() => short.to_owned(),
            _ => self.model_id.clone(),
        }
    }

    /// Where the installed files are below `root`.
    #[must_use]
    pub fn installed(&self, root: &Path) -> InstalledGguf {
        let path = |filename: &str| self.path(root, filename).to_string_lossy().into_owned();
        InstalledGguf {
            model_path: path(&self.model_file),
            mmproj_path: self.mmproj_file.as_deref().map(path),
            mtp_path: self.mtp_file.as_deref().map(path),
        }
    }

    /// Where `filename` of this download lands below `root`.
    #[must_use]
    pub fn path(&self, root: &Path, filename: &str) -> PathBuf {
        self.local_segments(filename)
            .iter()
            .fold(root.to_path_buf(), |path, segment| path.join(segment))
    }
}

impl HuggingFaceBrowser {
    /// A download client signed in with the saved token, for gated
    /// repositories.
    pub async fn download_client<S: SecretStore + ?Sized>(
        secrets: &S,
    ) -> Result<lettuce_network::ArtifactDownloadClient, HfBrowseError> {
        let client = lettuce_network::ArtifactDownloadClient::new().map_err(|error| {
            HfBrowseError::Message(format!("Failed to build download client: {error}"))
        })?;
        Ok(match Self::saved_token(secrets).await? {
            Some(token) => client.with_hugging_face_token(token),
            None => client,
        })
    }

    /// The download pinned to the repository's current revision, sizes and
    /// digests, below `root`.
    pub async fn gguf_install_plan<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        root: &Path,
        download: &GgufDownload,
    ) -> Result<ArtifactInstallPlan, HfBrowseError> {
        let token = Self::saved_token(secrets).await?;
        let response = self
            .get(
                &lettuce_model_hub::model_pin_request(&download.model_id),
                token.as_ref(),
            )
            .await
            .map_err(|error| {
                HfBrowseError::Message(format!("Failed to fetch model detail: {error}"))
            })?;
        if let Some(error) = lettuce_model_hub::access_error(
            response.status,
            HfResource::Model,
            &download.model_id,
            token.is_some(),
        ) {
            return Err(error);
        }
        if !(200..300).contains(&response.status) {
            return Err(HfBrowseError::Message(format!(
                "Model not found ({}): {}",
                lettuce_network::status_text(response.status),
                download.model_id
            )));
        }
        let pinned = lettuce_model_hub::pinned_files(
            &download.model_id,
            &response.body,
            &download.filenames(),
        )?;
        let artifacts = pinned
            .files
            .into_iter()
            .map(|file| {
                let source = ArtifactSource::HuggingFace {
                    repository: download.model_id.clone(),
                    revision: pinned.revision.clone(),
                    path: file.path.clone(),
                };
                PlannedArtifact {
                    artifact: PinnedArtifact {
                        source_identity: source.identity(),
                        local_segments: download.local_segments(&file.path),
                        byte_size: file.size,
                        sha256: file.sha256,
                    },
                    source,
                }
            })
            .collect();
        Ok(ArtifactInstallPlan {
            install_id: format!("hf-gguf:{}:{}", download.model_id, download.model_file),
            root: root.to_path_buf(),
            artifacts,
        })
    }
}

/// How a downloaded GGUF model is set up, as chosen when it was queued.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GgufModelSetup {
    pub display_name: Option<String>,
    pub context_length: Option<u64>,
    pub kv_type: Option<String>,
    pub offload_kqv: Option<bool>,
    pub gpu_layers: Option<u32>,
    pub model_offload: GgufModelOffload,
    /// The model file carries its own MTP head.
    pub mtp_bundled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GgufModelOffload {
    #[default]
    Auto,
    Cpu,
    Gpu,
    Mixed,
}

/// The installed files of a download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledGguf {
    pub model_path: String,
    pub mmproj_path: Option<String>,
    pub mtp_path: Option<String>,
}

impl GgufModelSetup {
    fn advanced_model_settings(&self, installed: &InstalledGguf) -> Map<String, Value> {
        let gpu_layers = match self.model_offload {
            GgufModelOffload::Cpu => Some(0),
            GgufModelOffload::Gpu => self.gpu_layers,
            GgufModelOffload::Auto | GgufModelOffload::Mixed => None,
        };
        let context_length = self
            .context_length
            .filter(|length| *length > 0)
            .unwrap_or(8192);
        let settings = json!({
            "maxOutputTokens": 2048,
            "contextLength": context_length,
            "llamaKvType": self.kv_type.as_deref().filter(|kv| !kv.is_empty()).unwrap_or("q8_0"),
            "llamaGpuLayers": gpu_layers,
            "llamaOffloadKqv": self.offload_kqv,
            "llamaMmprojPath": installed.mmproj_path,
            "llamaMtpEnabled": (installed.mtp_path.is_some() || self.mtp_bundled).then_some(true),
            "llamaMtpModelPath": installed.mtp_path,
        });
        match settings {
            Value::Object(settings) => settings,
            _ => Map::new(),
        }
    }
}

/// The managed llama.cpp account, created or re-enabled.
pub fn local_llama_account<R>(
    repository: &R,
    now: TimestampMillis,
) -> Result<ProviderAccount, ModelFileError>
where
    R: ModelLookup + ProviderAccountRepository + ?Sized,
{
    Ok(
        match repository
            .account_by_kind_and_label(LOCAL_LLAMA_PROVIDER_KIND, LOCAL_LLAMA_PROVIDER_LABEL)?
        {
            Some(account) if account.enabled => account,
            Some(account) => {
                let revision = account.revision;
                ProviderAccountRepository::upsert(
                    repository,
                    ProviderAccount {
                        enabled: true,
                        updated_at: now,
                        ..account
                    },
                    Some(revision),
                )?
            }
            None => ProviderAccountRepository::upsert(
                repository,
                ProviderAccount {
                    id: ProviderAccountId::new(),
                    secret_owner_id: SecretOwnerId::new(),
                    provider_kind: LOCAL_LLAMA_PROVIDER_KIND.to_owned(),
                    protocol: ProviderProtocol::LlamaCpp,
                    label: LOCAL_LLAMA_PROVIDER_LABEL.to_owned(),
                    endpoint: None,
                    enabled: true,
                    streaming_enabled: true,
                    allow_invalid_tls: false,
                    api_key_ref: None,
                    secret_headers: Vec::new(),
                    config: ProviderConfig::Standard,
                    revision: Revision::INITIAL,
                    created_at: now,
                    updated_at: now,
                },
                None,
            )?,
        },
    )
}

/// Adds the downloaded model as a llama.cpp chat model: image input when it
/// has a projector, MTP when it has a draft model or carries its own head.
pub fn register_downloaded_gguf<R>(
    repository: &R,
    root: &Path,
    download: &GgufDownload,
    setup: &GgufModelSetup,
    now: TimestampMillis,
) -> Result<ImportedModelFile, ModelFileError>
where
    R: lettuce_models::ModelCatalog
        + lettuce_settings::GlobalSettingsStore
        + ModelLookup
        + ModelProfileRepository
        + ProviderAccountRepository
        + ?Sized,
{
    local_llama_account(repository, now)?;
    let installed = download.installed(root);
    let input_scopes = if installed.mmproj_path.is_some() {
        vec!["text".to_owned(), "image".to_owned()]
    } else {
        vec!["text".to_owned()]
    };
    ModelFileCoordinator::new(repository).create(
        lettuce_transfer::ImportedModel {
            name: installed.model_path.clone(),
            provider_id: LOCAL_LLAMA_PROVIDER_KIND.to_owned(),
            provider_label: LOCAL_LLAMA_PROVIDER_LABEL.to_owned(),
            display_name: setup
                .display_name
                .clone()
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| download.default_display_name()),
            input_scopes,
            output_scopes: vec!["text".to_owned()],
            advanced_model_settings: setup.advanced_model_settings(&installed),
        },
        now,
    )
}

#[cfg(test)]
mod tests {
    use lettuce_models::ModelProfileRepository;
    use lettuce_settings::InMemorySecretStore;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    fn download() -> GgufDownload {
        GgufDownload {
            model_id: "org/Model-GGUF".to_owned(),
            model_file: "Q4/Model-Q4_K_M.GGUF".to_owned(),
            mmproj_file: Some("mmproj-F16.gguf".to_owned()),
            mtp_file: None,
        }
    }

    #[tokio::test]
    async fn a_download_is_pinned_below_the_models_folder() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).await.expect("read");
            assert!(
                String::from_utf8_lossy(&buffer[..read])
                    .starts_with("GET /api/models/org/Model-GGUF?blobs=true ")
            );
            let body = r#"{"sha": "d24c4cf2a0cd98a42f23467e27e3d76ee9438b8e", "siblings": [
                {"rfilename": "Q4/Model-Q4_K_M.GGUF", "size": 9, "lfs": {"size": 9, "sha256": "aa"}},
                {"rfilename": "mmproj-F16.gguf", "size": 4, "lfs": {"size": 4, "sha256": "bb"}}
            ]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.expect("write");
        });
        let browser = HuggingFaceBrowser::with_endpoint(
            lettuce_network::JsonClient::new().expect("client"),
            format!("http://{address}"),
        );
        let root = Path::new("/models");
        let plan = browser
            .gguf_install_plan(&InMemorySecretStore::default(), root, &download())
            .await
            .expect("plan");
        assert_eq!(
            plan.install_id,
            "hf-gguf:org/Model-GGUF:Q4/Model-Q4_K_M.GGUF"
        );
        assert_eq!(plan.total_bytes(), 13);
        assert_eq!(
            plan.artifacts[0].artifact.local_segments,
            ["org--Model-GGUF", "Q4", "Model-Q4_K_M.GGUF"]
        );
        assert_eq!(
            plan.artifacts[1].artifact.source_identity,
            "hf:org/Model-GGUF@d24c4cf2a0cd98a42f23467e27e3d76ee9438b8e/mmproj-F16.gguf"
        );
    }

    #[test]
    fn a_finished_download_becomes_a_local_llama_model() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        let root = Path::new("/models");
        let setup = GgufModelSetup {
            model_offload: GgufModelOffload::Cpu,
            mtp_bundled: true,
            ..GgufModelSetup::default()
        };
        let created =
            register_downloaded_gguf(database, root, &download(), &setup, TimestampMillis::new(5))
                .expect("model");
        let profile = created.profile;
        assert_eq!(profile.display_name, "Model-Q4_K_M");
        assert_eq!(
            lettuce_settings::GlobalSettingsStore::load(database)
                .expect("settings")
                .default_model_profile_id,
            Some(profile.id)
        );
        assert_eq!(
            profile.external_model_id,
            download()
                .path(root, "Q4/Model-Q4_K_M.GGUF")
                .to_string_lossy()
        );
        let llama = &profile.config.llama_cpp;
        assert_eq!(llama.gpu_layers, Some(0));
        assert_eq!(llama.mtp_enabled, Some(true));
        assert_eq!(
            llama.mmproj_path.as_deref(),
            Some(
                download()
                    .path(root, "mmproj-F16.gguf")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert_eq!(profile.config.chat_parameters.context_length, Some(8192));
        assert_eq!(
            profile.config.capabilities.input_modalities.image,
            lettuce_models::CapabilityStatus::Supported
        );
        let again = register_downloaded_gguf(
            database,
            root,
            &GgufDownload {
                mmproj_file: None,
                ..download()
            },
            &GgufModelSetup {
                display_name: Some("Mine".to_owned()),
                context_length: Some(32768),
                kv_type: Some("q4_0".to_owned()),
                gpu_layers: Some(20),
                model_offload: GgufModelOffload::Mixed,
                ..GgufModelSetup::default()
            },
            TimestampMillis::new(6),
        )
        .expect("second model");
        assert_eq!(
            again.profile.provider_account_id,
            profile.provider_account_id
        );
        assert_eq!(again.profile.display_name, "Mine");
        assert_eq!(again.profile.config.llama_cpp.gpu_layers, None);
        assert_eq!(again.profile.config.llama_cpp.mtp_enabled, None);
        assert_eq!(
            ModelProfileRepository::get(database, again.profile.id)
                .expect("get")
                .expect("stored")
                .config
                .chat_parameters
                .context_length,
            Some(32768)
        );
    }
}
