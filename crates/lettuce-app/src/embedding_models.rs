//! Memory embedding models (Lettuce Eidos, and the deprecated v4): resolved
//! from the Hugging Face API and pinned to the commit it reports, installed
//! as one resumable `ArtifactInstall` job (progress and cancellation come
//! from the job), then recorded, chosen, loaded and removed below
//! `<app folder>/models/embedding`, the legacy embedding folder.

use std::path::{Path, PathBuf};

use lettuce_embeddings::{EmbeddingDimensions, OnnxRuntimeLink, SimilarityCalibration};
use lettuce_model_hub::{
    EmbeddingArtifactRole, EmbeddingFileDigest, EmbeddingInstallError, EmbeddingInstallStore,
    EmbeddingModelFamily, EmbeddingPin, HUGGING_FACE_ENDPOINT, InstalledEmbeddingManifest,
    InstalledModelArtifact, ModelArtifactError, PinnedArtifact, PinnedArtifactError,
    inspect_legacy_embedding_install, parse_embedding_pin, select_embedding_family,
    verify_git_blob,
};
use lettuce_network::{JsonAuth, JsonClient, JsonClientError, JsonQueryParameter, RequestPolicy};
use lettuce_settings::{DeviceSettingsStore, EmbeddingModelVersion, GlobalSettingsStoreError};

use crate::{
    ArtifactInstallPlan, ArtifactSource, EmbeddingService, EmbeddingServiceError, PlannedArtifact,
};

#[must_use]
pub fn embedding_models_root(app_folder: &Path) -> PathBuf {
    app_folder.join("models").join("embedding")
}

/// The family a stored model choice loads; v3 has no runtime and falls back
/// to whichever family is installed.
#[must_use]
pub const fn embedding_family_for_version(
    version: EmbeddingModelVersion,
) -> Option<EmbeddingModelFamily> {
    match version {
        EmbeddingModelVersion::V3 => None,
        EmbeddingModelVersion::V4 => Some(EmbeddingModelFamily::LettuceEmbV4),
        EmbeddingModelVersion::V5 => Some(EmbeddingModelFamily::LettuceEidosV5),
    }
}

#[must_use]
pub const fn embedding_version_for_family(family: EmbeddingModelFamily) -> EmbeddingModelVersion {
    match family {
        EmbeddingModelFamily::LettuceEmbV4 => EmbeddingModelVersion::V4,
        EmbeddingModelFamily::LettuceEidosV5 => EmbeddingModelVersion::V5,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbeddingModelError {
    #[error("the embedding model lookup failed: {0}")]
    Network(JsonClientError),
    #[error("the embedding model lookup returned status {0}")]
    Response(u16),
    #[error("{0}")]
    Install(#[from] EmbeddingInstallError),
    #[error("the embedding model's calibration.json is invalid")]
    InvalidCalibration,
    #[error("the embedding model is not installed")]
    NotInstalled,
    #[error("device settings failed: {0}")]
    Settings(#[from] GlobalSettingsStoreError),
}

#[derive(Debug, Clone)]
pub struct EmbeddingModelCatalog {
    client: JsonClient,
    endpoint: String,
}

impl EmbeddingModelCatalog {
    #[must_use]
    pub fn new(client: JsonClient) -> Self {
        Self::with_endpoint(client, HUGGING_FACE_ENDPOINT)
    }

    #[must_use]
    pub fn with_endpoint(client: JsonClient, endpoint: impl Into<String>) -> Self {
        Self {
            client,
            endpoint: endpoint.into(),
        }
    }

    /// The family's current files, pinned to the commit the API reports.
    pub async fn pin(
        &self,
        family: EmbeddingModelFamily,
    ) -> Result<EmbeddingPin, EmbeddingModelError> {
        let request = lettuce_model_hub::model_pin_request(family.repository());
        let query = request
            .query
            .iter()
            .map(|(name, value)| JsonQueryParameter {
                name: name.as_str(),
                value: value.as_str(),
            })
            .collect::<Vec<_>>();
        let response = self
            .client
            .get_json_with_query(
                &self.endpoint,
                &request.path,
                &query,
                &[],
                JsonAuth::None,
                Vec::new(),
                RequestPolicy::PROBE,
            )
            .await
            .map_err(EmbeddingModelError::Network)?;
        if response.status != 200 {
            return Err(EmbeddingModelError::Response(response.status));
        }
        Ok(parse_embedding_pin(family, &response.body)?)
    }
}

/// Every pinned file below `root`; LFS files are checked against their
/// SHA-256 while downloading, plain git files by size, and their git blob id
/// when the install is recorded.
#[must_use]
pub fn embedding_install_plan(root: &Path, pin: &EmbeddingPin) -> ArtifactInstallPlan {
    let artifacts = pin
        .files
        .iter()
        .map(|file| {
            let source = ArtifactSource::HuggingFace {
                repository: pin.family.repository().to_owned(),
                revision: pin.revision.clone(),
                path: file.remote_path.clone(),
            };
            PlannedArtifact {
                artifact: PinnedArtifact {
                    source_identity: source.identity(),
                    local_segments: pin.local_segments(file),
                    byte_size: file.byte_size,
                    sha256: match &file.digest {
                        EmbeddingFileDigest::Sha256(sha256) => Some(sha256.clone()),
                        EmbeddingFileDigest::GitBlobSha1(_) => None,
                    },
                },
                source,
            }
        })
        .collect();
    ArtifactInstallPlan {
        install_id: format!("embedding:{}@{}", pin.family.repository(), pin.revision),
        root: root.to_path_buf(),
        artifacts,
    }
}

fn discard(path: &Path) {
    if let Err(error) = std::fs::remove_file(path) {
        tracing::warn!(%error, "unverified embedding file could not be removed");
    }
}

/// Verifies the files a succeeded install job wrote and records their
/// manifest. A plain git file that fails its blob id, or a calibration that
/// does not parse, is removed so the next install downloads it again.
pub fn finish_embedding_install(
    root: &Path,
    pin: &EmbeddingPin,
) -> Result<InstalledEmbeddingManifest, EmbeddingModelError> {
    let mut model = None;
    let mut tokenizer = None;
    let mut calibration = None;
    for file in &pin.files {
        let path = pin
            .local_segments(file)
            .iter()
            .fold(root.to_path_buf(), |path, segment| path.join(segment));
        if let EmbeddingFileDigest::GitBlobSha1(blob) = &file.digest
            && let Err(error) = verify_git_blob(&path, blob)
        {
            discard(&path);
            return Err(EmbeddingInstallError::Artifact(match error {
                PinnedArtifactError::Unreadable => ModelArtifactError::Unreadable,
                _ => ModelArtifactError::Mismatch,
            })
            .into());
        }
        let artifact =
            InstalledModelArtifact::inspect(path.clone()).map_err(EmbeddingInstallError::from)?;
        if artifact.byte_size != file.byte_size {
            return Err(EmbeddingInstallError::PinMismatch.into());
        }
        match file.role {
            EmbeddingArtifactRole::Model => model = Some(artifact),
            EmbeddingArtifactRole::Tokenizer => tokenizer = Some(artifact),
            EmbeddingArtifactRole::Calibration => {
                let bytes = std::fs::read(&path).map_err(|_| EmbeddingInstallError::Storage)?;
                if SimilarityCalibration::from_json(&bytes).is_err() {
                    discard(&path);
                    return Err(EmbeddingModelError::InvalidCalibration);
                }
                calibration = Some(artifact);
            }
        }
    }
    let manifest = InstalledEmbeddingManifest {
        family: pin.family,
        source_revision: pin.revision.clone(),
        model: model.ok_or(EmbeddingInstallError::PinMismatch)?,
        tokenizer: tokenizer.ok_or(EmbeddingInstallError::PinMismatch)?,
        calibration,
        max_sequence_length: pin.family.max_positions(),
        native_dimensions: pin.family.native_dimensions(),
    };
    EmbeddingInstallStore::new(root).record(&manifest)?;
    Ok(manifest)
}

/// Which embedding model is installed and loads, and the user's choice of
/// it (device settings `embedding.model_version`).
#[derive(Debug)]
pub struct EmbeddingModelCoordinator<'a, S: ?Sized> {
    store: EmbeddingInstallStore,
    settings: &'a S,
}

impl<'a, S: DeviceSettingsStore + ?Sized> EmbeddingModelCoordinator<'a, S> {
    #[must_use]
    pub fn new(root: &Path, settings: &'a S) -> Self {
        Self {
            store: EmbeddingInstallStore::new(root),
            settings,
        }
    }

    pub fn installed(&self) -> Result<Vec<InstalledEmbeddingManifest>, EmbeddingModelError> {
        Ok(self.store.installed()?)
    }

    /// The install that loads: the chosen family when installed (Eidos when
    /// nothing was chosen), else any installed family, Eidos first.
    pub fn active(&self) -> Result<Option<InstalledEmbeddingManifest>, EmbeddingModelError> {
        let installed = self.store.installed()?;
        let preferred = embedding_family_for_version(
            self.settings
                .load_device_settings()?
                .embedding
                .preferred_model_version(),
        );
        let families = installed
            .iter()
            .map(|manifest| manifest.family)
            .collect::<Vec<_>>();
        Ok(
            select_embedding_family(preferred, &families).and_then(|family| {
                installed
                    .into_iter()
                    .find(|manifest| manifest.family == family)
            }),
        )
    }

    /// Makes an installed family the one that loads; each memory space is
    /// re-embedded in its space the next time retrieval needs it.
    pub fn choose(&self, family: EmbeddingModelFamily) -> Result<(), EmbeddingModelError> {
        if self.store.manifest(family)?.is_none() {
            return Err(EmbeddingModelError::NotInstalled);
        }
        let mut settings = self.settings.load_device_settings()?;
        settings.embedding.model_version = Some(embedding_version_for_family(family));
        Ok(self.settings.save_device_settings(settings)?)
    }

    /// Records a succeeded install and chooses it, as a download also made
    /// the downloaded version the chosen one.
    pub fn complete_install(
        &self,
        pin: &EmbeddingPin,
    ) -> Result<InstalledEmbeddingManifest, EmbeddingModelError> {
        let manifest = finish_embedding_install(self.store.root(), pin)?;
        self.choose(manifest.family)?;
        Ok(manifest)
    }

    /// `false` when the family was not installed. The stored choice is kept,
    /// so loading falls back to another installed family.
    pub fn remove(&self, family: EmbeddingModelFamily) -> Result<bool, EmbeddingModelError> {
        Ok(self.store.remove(family)?)
    }

    /// Records a legacy v4 install found in `legacy_root` over its files
    /// where they are, unless v4 is already recorded. A device that never
    /// chose a model keeps loading v4, so its memories are not re-embedded
    /// until Eidos is downloaded.
    pub fn adopt_legacy_install(
        &self,
        legacy_root: &Path,
    ) -> Result<Option<InstalledEmbeddingManifest>, EmbeddingModelError> {
        if self
            .store
            .manifest(EmbeddingModelFamily::LettuceEmbV4)?
            .is_some()
        {
            return Ok(None);
        }
        let Some(manifest) =
            inspect_legacy_embedding_install(legacy_root).map_err(EmbeddingInstallError::from)?
        else {
            return Ok(None);
        };
        self.store.record(&manifest)?;
        let mut settings = self.settings.load_device_settings()?;
        if settings.embedding.model_version.is_none() {
            settings.embedding.model_version = Some(EmbeddingModelVersion::V4);
            self.settings.save_device_settings(settings)?;
        }
        Ok(Some(manifest))
    }

    /// The service of the active install at the user's dimension and token
    /// budget, or `None` when no embedding model is installed.
    pub fn load_active(
        &self,
        runtime_link: &OnnxRuntimeLink,
        dimensions: Option<u16>,
    ) -> Result<Option<EmbeddingService>, EmbeddingModelLoadError> {
        let Some(manifest) = self.active()? else {
            return Ok(None);
        };
        let max_tokens = self
            .settings
            .load_device_settings()
            .map_err(EmbeddingModelError::from)?
            .embedding
            .max_tokens;
        EmbeddingService::load(
            &manifest,
            runtime_link,
            EmbeddingDimensions::from_preference(dimensions),
            max_tokens,
        )
        .map(Some)
        .map_err(Into::into)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingModelLoadError {
    #[error("{0}")]
    Model(#[from] EmbeddingModelError),
    #[error("{0}")]
    Service(#[from] EmbeddingServiceError),
}

#[cfg(test)]
mod tests {
    use std::{sync::Mutex, time::Duration};

    use async_trait::async_trait;
    use lettuce_database::Database;
    use lettuce_jobs::{CancellationReason, JobState, ResourceAvailability, WorkerId};
    use lettuce_settings::DeviceSettings;
    use lettuce_types::{OperationId, TimestampMillis};
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::{
        ArtifactBody, ArtifactInstallCoordinator, ArtifactInstallRunResult, ArtifactSourceClient,
        ArtifactSourceError,
    };

    const REVISION: &str = "f14e5de6ab468df6651b6505f59a7224e630ee79";
    const CALIBRATION: &str = r#"{"default_threshold": 0.5, "fallback_threshold": 0.35, "dims": {
        "768": {"a": 2.381, "b": -1.381}, "512": {"a": 2.2901, "b": -1.2863},
        "384": {"a": 2.2556, "b": -1.2549}, "256": {"a": 2.2388, "b": -1.244},
        "128": {"a": 2.2388, "b": -1.2664}, "64": {"a": 1.9481, "b": -1.0058}}}"#;

    #[derive(Debug, Default)]
    struct Settings(Mutex<DeviceSettings>);

    impl DeviceSettingsStore for Settings {
        fn load_device_settings(&self) -> Result<DeviceSettings, GlobalSettingsStoreError> {
            Ok(self.0.lock().expect("settings").clone())
        }

        fn save_device_settings(
            &self,
            settings: DeviceSettings,
        ) -> Result<(), GlobalSettingsStoreError> {
            *self.0.lock().expect("settings") = settings;
            Ok(())
        }
    }

    fn git_blob(bytes: &[u8]) -> String {
        let mut hasher = sha1_smol::Sha1::new();
        hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
        hasher.update(bytes);
        hasher.digest().to_string()
    }

    fn files(calibration: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("onnx/model_quantized.onnx", b"eidos model bytes".to_vec()),
            ("tokenizer.json", b"eidos tokenizer".to_vec()),
            ("calibration.json", calibration.to_vec()),
        ]
    }

    fn detail(files: &[(&str, Vec<u8>)], listed_calibration: &[u8]) -> Vec<u8> {
        let siblings = files
            .iter()
            .map(|(path, bytes)| {
                if *path == "calibration.json" {
                    serde_json::json!({
                        "rfilename": path,
                        "size": bytes.len(),
                        "blobId": git_blob(listed_calibration)
                    })
                } else {
                    serde_json::json!({
                        "rfilename": path,
                        "size": bytes.len(),
                        "lfs": { "size": bytes.len(), "sha256": format!("{:x}", Sha256::digest(bytes)) }
                    })
                }
            })
            .collect::<Vec<_>>();
        serde_json::to_vec(&serde_json::json!({ "sha": REVISION, "siblings": siblings }))
            .expect("detail")
    }

    struct Body {
        bytes: Vec<u8>,
        start: u64,
        sent: bool,
    }

    #[async_trait]
    impl ArtifactBody for Body {
        fn start(&self) -> u64 {
            self.start
        }

        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ArtifactSourceError> {
            if self.sent {
                return Ok(None);
            }
            self.sent = true;
            Ok(Some(
                self.bytes[usize::try_from(self.start).expect("offset")..].to_vec(),
            ))
        }
    }

    struct Source(Vec<(String, Vec<u8>)>);

    #[async_trait]
    impl ArtifactSourceClient for Source {
        async fn open(
            &self,
            source: &ArtifactSource,
            offset: u64,
            _expected_size: u64,
        ) -> Result<Box<dyn ArtifactBody>, ArtifactSourceError> {
            let ArtifactSource::HuggingFace {
                repository,
                revision,
                path,
            } = source
            else {
                return Err(ArtifactSourceError::InvalidRequest);
            };
            if repository != "Zeolit/lettuce-eidos-768d-v5" || revision != REVISION {
                return Err(ArtifactSourceError::InvalidRequest);
            }
            let (_, bytes) = self
                .0
                .iter()
                .find(|(candidate, _)| candidate == path)
                .ok_or(ArtifactSourceError::InvalidRequest)?;
            Ok(Box::new(Body {
                bytes: bytes.clone(),
                start: offset,
                sent: false,
            }))
        }
    }

    async fn install(root: &Path, pin: &EmbeddingPin, files: Vec<(&str, Vec<u8>)>) -> JobState {
        let database = Database::open_in_memory().expect("database");
        let plan = embedding_install_plan(root, pin);
        let coordinator = ArtifactInstallCoordinator::new(&database);
        let admitted = coordinator.admit(&plan).expect("admit");
        let work = coordinator
            .claim(
                plan,
                admitted.job.id,
                WorkerId::new(),
                TimestampMillis::new(1),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        let source = Source(
            files
                .into_iter()
                .map(|(path, bytes)| (path.to_owned(), bytes))
                .collect(),
        );
        match coordinator
            .run(
                work,
                &source,
                CancellationReason::User,
                TimestampMillis::new(2),
            )
            .await
            .expect("run")
        {
            ArtifactInstallRunResult::Succeeded { job, .. }
            | ArtifactInstallRunResult::Cancelled { job }
            | ArtifactInstallRunResult::Failed { job, .. } => job.state,
        }
    }

    #[tokio::test]
    async fn eidos_installs_pinned_with_a_verified_calibration_and_becomes_the_choice() {
        let root = std::env::temp_dir().join(format!("embedding-install-{}", OperationId::new()));
        let files = files(CALIBRATION.as_bytes());
        let pin = parse_embedding_pin(
            EmbeddingModelFamily::LettuceEidosV5,
            &detail(&files, CALIBRATION.as_bytes()),
        )
        .expect("pin");
        let plan = embedding_install_plan(&root, &pin);
        assert_eq!(plan.artifacts.len(), 3);
        assert!(plan.artifacts[2].artifact.sha256.is_none());
        assert_eq!(install(&root, &pin, files).await, JobState::Succeeded);

        let settings = Settings(Mutex::new(DeviceSettings {
            embedding: lettuce_settings::DeviceEmbeddingSettings {
                model_version: Some(EmbeddingModelVersion::V4),
                ..Default::default()
            },
            ..DeviceSettings::default()
        }));
        let coordinator = EmbeddingModelCoordinator::new(&root, &settings);
        assert_eq!(coordinator.active(), Ok(None));
        let manifest = coordinator.complete_install(&pin).expect("recorded");
        assert_eq!(manifest.source_revision, REVISION);
        assert_eq!(manifest.max_sequence_length, 4096);
        assert!(manifest.calibration.is_some());
        assert_eq!(
            settings.0.lock().expect("settings").embedding.model_version,
            Some(EmbeddingModelVersion::V5)
        );
        assert_eq!(coordinator.active(), Ok(Some(manifest)));
        assert_eq!(
            coordinator.remove(EmbeddingModelFamily::LettuceEidosV5),
            Ok(true)
        );
        assert_eq!(coordinator.active(), Ok(None));
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn a_calibration_that_fails_its_blob_id_or_parse_is_refused_and_removed() {
        let root = std::env::temp_dir().join(format!("embedding-install-{}", OperationId::new()));
        let tampered = CALIBRATION.replace("2.381", "9.999");
        let files = files(tampered.as_bytes());
        let pin = parse_embedding_pin(
            EmbeddingModelFamily::LettuceEidosV5,
            &detail(&files, CALIBRATION.as_bytes()),
        )
        .expect("pin");
        assert_eq!(install(&root, &pin, files).await, JobState::Succeeded);
        let calibration = root
            .join("lettuce-eidos-768d-v5")
            .join(REVISION)
            .join("calibration.json");
        assert!(calibration.exists());
        assert!(matches!(
            finish_embedding_install(&root, &pin),
            Err(EmbeddingModelError::Install(
                EmbeddingInstallError::Artifact(_)
            ))
        ));
        assert!(!calibration.exists());

        let invalid = br#"{"default_threshold": 0.5, "fallback_threshold": 0.35, "dims": {}}"#;
        let files = self::files(invalid);
        let pin = parse_embedding_pin(
            EmbeddingModelFamily::LettuceEidosV5,
            &detail(&files, invalid),
        )
        .expect("pin");
        assert_eq!(install(&root, &pin, files).await, JobState::Succeeded);
        assert_eq!(
            finish_embedding_install(&root, &pin),
            Err(EmbeddingModelError::InvalidCalibration)
        );
        assert!(!calibration.exists());
        assert_eq!(
            EmbeddingInstallStore::new(&root).installed(),
            Ok(Vec::new())
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn legacy_v4_files_are_adopted_and_keep_an_unchosen_device_on_v4() {
        let root = std::env::temp_dir().join(format!("embedding-legacy-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        std::fs::write(root.join("v4-model.int8.onnx"), b"v4 model").expect("model");
        std::fs::write(root.join("v4-tokenizer.json"), b"v4 tokenizer").expect("tokenizer");
        let settings = Settings::default();
        let coordinator = EmbeddingModelCoordinator::new(&root, &settings);
        let adopted = coordinator
            .adopt_legacy_install(&root)
            .expect("adopt")
            .expect("legacy install");
        assert_eq!(adopted.family, EmbeddingModelFamily::LettuceEmbV4);
        assert_eq!(
            settings.0.lock().expect("settings").embedding.model_version,
            Some(EmbeddingModelVersion::V4)
        );
        assert_eq!(coordinator.adopt_legacy_install(&root), Ok(None));
        assert_eq!(coordinator.active(), Ok(Some(adopted)));
        assert_eq!(
            coordinator.choose(EmbeddingModelFamily::LettuceEidosV5),
            Err(EmbeddingModelError::NotInstalled)
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    #[ignore = "downloads the pinned Eidos listing from Hugging Face"]
    fn live_eidos_listing_pins_every_file_with_a_digest() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let pin = runtime
            .block_on(
                EmbeddingModelCatalog::new(JsonClient::new().expect("client"))
                    .pin(EmbeddingModelFamily::LettuceEidosV5),
            )
            .expect("pin");
        assert_eq!(pin.revision.len(), 40);
        assert_eq!(pin.files.len(), 3);
        assert!(matches!(
            pin.files[2].digest,
            EmbeddingFileDigest::GitBlobSha1(_)
        ));
    }

    #[test]
    #[ignore = "downloads Eidos from Hugging Face and needs ONNX Runtime"]
    fn live_eidos_download_installs_loads_and_scores_with_its_calibration() {
        let onnx_runtime = std::env::var_os("LETTUCE_TEST_ONNX_RUNTIME")
            .map(PathBuf::from)
            .expect("runtime path");
        let root = std::env::temp_dir().join(format!("eidos-live-{}", OperationId::new()));
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let pin = runtime
            .block_on(
                EmbeddingModelCatalog::new(JsonClient::new().expect("client"))
                    .pin(EmbeddingModelFamily::LettuceEidosV5),
            )
            .expect("pin");
        let database = Database::open_in_memory().expect("database");
        let plan = embedding_install_plan(&root, &pin);
        let coordinator = ArtifactInstallCoordinator::new(&database);
        let admitted = coordinator.admit(&plan).expect("admit");
        let work = coordinator
            .claim(
                plan,
                admitted.job.id,
                WorkerId::new(),
                TimestampMillis::new(1),
                Duration::from_secs(600),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        let client = lettuce_network::ArtifactDownloadClient::new().expect("download client");
        let result = runtime
            .block_on(coordinator.run(
                work,
                &client,
                CancellationReason::User,
                TimestampMillis::new(2),
            ))
            .expect("run");
        assert!(matches!(result, ArtifactInstallRunResult::Succeeded { .. }));
        let settings = Settings::default();
        let models = EmbeddingModelCoordinator::new(&root, &settings);
        models.complete_install(&pin).expect("recorded");
        let service = models
            .load_active(&OnnxRuntimeLink::Dynamic(onnx_runtime), Some(256))
            .expect("load")
            .expect("installed");
        assert_eq!(
            crate::MemoryEmbeddingEngine::source_revision(&service),
            "v5"
        );
        let embed = |text: &str| {
            service
                .embed(
                    &lettuce_embeddings::EmbeddingRequest {
                        text: text.to_owned(),
                        dimensions: EmbeddingDimensions::D256,
                    },
                    &lettuce_jobs::handle::CancellationToken::new(),
                )
                .expect("embedding")
        };
        let query = embed("Do you remember where you hid the sword?");
        let related = embed("Elara hid the sword in the old stone well behind the mill.");
        let unrelated = embed("Apple prices at the market went up again this week.");
        let calibration = crate::MemoryEmbeddingEngine::calibration(&service);
        let shown = |other| {
            calibration.score(
                query.cosine_similarity(other).expect("cosine"),
                EmbeddingDimensions::D256,
            )
        };
        assert!(shown(&related) >= 0.5);
        assert!(shown(&unrelated) < 0.35);
        std::fs::remove_dir_all(root).ok();
    }
}
