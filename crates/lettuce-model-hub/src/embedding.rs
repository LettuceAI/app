use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use lettuce_types::ContentHash;
use serde::{Deserialize, Serialize};

pub(crate) const MAX_SOURCE_REVISION_BYTES: usize = 128;
const MAX_EMBEDDING_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MANIFEST_FILE: &str = "manifest.json";
const LEGACY_V4_MODEL_FILE: &str = "v4-model.int8.onnx";
const LEGACY_V4_TOKENIZER_FILE: &str = "v4-tokenizer.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingModelFamily {
    LettuceEmbV4,
    LettuceEidosV5,
}

/// What a file of an embedding repository is used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingArtifactRole {
    Model,
    Tokenizer,
    /// The raw-cosine to shown-score mapping a family publishes next to its
    /// weights.
    Calibration,
}

/// One repository file an install downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddingRemoteFile {
    pub role: EmbeddingArtifactRole,
    pub remote_path: &'static str,
}

const V4_FILES: [EmbeddingRemoteFile; 2] = [
    EmbeddingRemoteFile {
        role: EmbeddingArtifactRole::Model,
        remote_path: "onnx/model.int8.onnx",
    },
    EmbeddingRemoteFile {
        role: EmbeddingArtifactRole::Tokenizer,
        remote_path: "tokenizer.json",
    },
];

const EIDOS_FILES: [EmbeddingRemoteFile; 3] = [
    EmbeddingRemoteFile {
        role: EmbeddingArtifactRole::Model,
        remote_path: "onnx/model_quantized.onnx",
    },
    EmbeddingRemoteFile {
        role: EmbeddingArtifactRole::Tokenizer,
        remote_path: "tokenizer.json",
    },
    EmbeddingRemoteFile {
        role: EmbeddingArtifactRole::Calibration,
        remote_path: "calibration.json",
    },
];

impl EmbeddingModelFamily {
    pub const ALL: [Self; 2] = [Self::LettuceEidosV5, Self::LettuceEmbV4];

    #[must_use]
    pub const fn repository(self) -> &'static str {
        match self {
            Self::LettuceEmbV4 => "Zeolit/lettuce-emb-768d-v4",
            Self::LettuceEidosV5 => "Zeolit/lettuce-eidos-768d-v5",
        }
    }

    /// The label stored with every vector this family produces. Vectors
    /// compare only within one label and dimension, whatever download or
    /// import the model files came from.
    #[must_use]
    pub const fn vector_space(self) -> &'static str {
        match self {
            Self::LettuceEmbV4 => "v4",
            Self::LettuceEidosV5 => "v5",
        }
    }

    /// The longest token sequence the model was trained on: v4's config
    /// declares 2,048 positions and Eidos was trained at 4,096.
    #[must_use]
    pub const fn max_positions(self) -> usize {
        match self {
            Self::LettuceEmbV4 => 2048,
            Self::LettuceEidosV5 => 4096,
        }
    }

    #[must_use]
    pub const fn native_dimensions(self) -> usize {
        768
    }

    /// Eidos raw cosines sit high for unrelated text, so its scores are only
    /// usable through the calibration it publishes.
    #[must_use]
    pub const fn requires_calibration(self) -> bool {
        matches!(self, Self::LettuceEidosV5)
    }

    /// Eidos writes its pooled, normalized vector to this output; v4 uses
    /// its first output.
    #[must_use]
    pub const fn output_name(self) -> Option<&'static str> {
        match self {
            Self::LettuceEmbV4 => None,
            Self::LettuceEidosV5 => Some("embedding"),
        }
    }

    #[must_use]
    pub const fn remote_files(self) -> &'static [EmbeddingRemoteFile] {
        match self {
            Self::LettuceEmbV4 => &V4_FILES,
            Self::LettuceEidosV5 => &EIDOS_FILES,
        }
    }

    /// The folder below the embedding root that holds this family's
    /// downloads and manifest.
    #[must_use]
    pub const fn install_dir(self) -> &'static str {
        match self {
            Self::LettuceEmbV4 => "lettuce-emb-768d-v4",
            Self::LettuceEidosV5 => "lettuce-eidos-768d-v5",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledModelArtifact {
    pub path: PathBuf,
    pub byte_size: u64,
    pub blake3: ContentHash,
}

impl InstalledModelArtifact {
    pub fn inspect(path: PathBuf) -> Result<Self, ModelArtifactError> {
        let metadata = std::fs::metadata(&path).map_err(|_| ModelArtifactError::Missing)?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_EMBEDDING_FILE_BYTES {
            return Err(ModelArtifactError::InvalidManifest);
        }
        let blake3 = hash_file(&path)?;
        Ok(Self {
            path,
            byte_size: metadata.len(),
            blake3,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledEmbeddingManifest {
    pub family: EmbeddingModelFamily,
    /// Immutable upstream commit, release, or imported legacy artifact ID.
    pub source_revision: String,
    pub model: InstalledModelArtifact,
    pub tokenizer: InstalledModelArtifact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<InstalledModelArtifact>,
    pub max_sequence_length: usize,
    pub native_dimensions: usize,
}

impl InstalledEmbeddingManifest {
    pub fn verify(&self) -> Result<VerifiedEmbeddingArtifacts, ModelArtifactError> {
        let revision = self.source_revision.trim();
        if revision.is_empty() || revision.len() > MAX_SOURCE_REVISION_BYTES {
            return Err(ModelArtifactError::InvalidManifest);
        }
        if self.max_sequence_length == 0
            || self.max_sequence_length > self.family.max_positions()
            || self.native_dimensions != self.family.native_dimensions()
            || self.calibration.is_some() != self.family.requires_calibration()
        {
            return Err(ModelArtifactError::InvalidManifest);
        }
        verify_artifact(&self.model)?;
        verify_artifact(&self.tokenizer)?;
        if let Some(calibration) = &self.calibration {
            verify_artifact(calibration)?;
        }
        Ok(VerifiedEmbeddingArtifacts {
            family: self.family,
            source_revision: revision.to_owned(),
            model_path: self.model.path.clone(),
            tokenizer_path: self.tokenizer.path.clone(),
            calibration_path: self
                .calibration
                .as_ref()
                .map(|calibration| calibration.path.clone()),
            max_sequence_length: self.max_sequence_length,
            native_dimensions: self.native_dimensions,
        })
    }

    fn artifacts(&self) -> impl Iterator<Item = &InstalledModelArtifact> {
        [&self.model, &self.tokenizer]
            .into_iter()
            .chain(self.calibration.as_ref())
    }
}

pub(crate) fn verify_artifact(artifact: &InstalledModelArtifact) -> Result<(), ModelArtifactError> {
    if artifact.byte_size == 0 || artifact.byte_size > MAX_EMBEDDING_FILE_BYTES {
        return Err(ModelArtifactError::InvalidManifest);
    }
    let metadata = std::fs::metadata(&artifact.path).map_err(|_| ModelArtifactError::Missing)?;
    if !metadata.is_file() || metadata.len() != artifact.byte_size {
        return Err(ModelArtifactError::Mismatch);
    }
    let actual = hash_file(&artifact.path)?;
    if actual != artifact.blake3 {
        return Err(ModelArtifactError::Mismatch);
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<ContentHash, ModelArtifactError> {
    let mut file = File::open(path).map_err(|_| ModelArtifactError::Unreadable)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| ModelArtifactError::Unreadable)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    ContentHash::parse(hasher.finalize().to_hex().to_string())
        .map_err(|_| ModelArtifactError::InvalidManifest)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedEmbeddingArtifacts {
    pub family: EmbeddingModelFamily,
    pub source_revision: String,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub calibration_path: Option<PathBuf>,
    pub max_sequence_length: usize,
    pub native_dimensions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelArtifactError {
    #[error("model artifact manifest is invalid")]
    InvalidManifest,
    #[error("model artifact is missing")]
    Missing,
    #[error("model artifact cannot be read")]
    Unreadable,
    #[error("model artifact does not match its verified manifest")]
    Mismatch,
}

/// How a pinned repository file is checked once downloaded: LFS files by
/// the SHA-256 Hugging Face lists, plain git files by their git blob id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingFileDigest {
    Sha256(String),
    GitBlobSha1(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingPinnedFile {
    pub role: EmbeddingArtifactRole,
    pub remote_path: String,
    pub byte_size: u64,
    pub digest: EmbeddingFileDigest,
}

impl EmbeddingPinnedFile {
    /// The file name the install stores the file under.
    #[must_use]
    pub fn local_name(&self) -> &str {
        self.remote_path
            .rsplit('/')
            .next()
            .unwrap_or(&self.remote_path)
    }
}

/// A family's files at the repository revision Hugging Face reported when
/// the install was planned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingPin {
    pub family: EmbeddingModelFamily,
    pub revision: String,
    pub files: Vec<EmbeddingPinnedFile>,
}

impl EmbeddingPin {
    /// Where a pinned file lands below the embedding root:
    /// `<family dir>/<revision>/<file name>`, so a newer revision never
    /// replaces the files of the one in use.
    #[must_use]
    pub fn local_segments(&self, file: &EmbeddingPinnedFile) -> Vec<String> {
        vec![
            self.family.install_dir().to_owned(),
            self.revision.clone(),
            file.local_name().to_owned(),
        ]
    }

    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.byte_size).sum()
    }
}

fn is_hex_digest(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The family's files in a `model_pin_request` response, read by the shared
/// `pinned_files`. Every file needs an LFS SHA-256 or a git blob id.
pub fn parse_embedding_pin(
    family: EmbeddingModelFamily,
    body: &[u8],
) -> Result<EmbeddingPin, EmbeddingInstallError> {
    let paths = family
        .remote_files()
        .iter()
        .map(|remote| remote.remote_path)
        .collect::<Vec<_>>();
    let pinned = crate::pinned_files(family.repository(), body, &paths)
        .map_err(|_| EmbeddingInstallError::InvalidPin)?;
    let files = family
        .remote_files()
        .iter()
        .zip(pinned.files)
        .map(|(remote, file)| {
            let digest = match (file.sha256, file.git_blob_id) {
                (Some(sha256), _) if is_hex_digest(&sha256, 64) => {
                    EmbeddingFileDigest::Sha256(sha256)
                }
                (None, Some(blob_id)) => EmbeddingFileDigest::GitBlobSha1(blob_id),
                _ => return Err(EmbeddingInstallError::InvalidPin),
            };
            if file.path != remote.remote_path || file.size > MAX_EMBEDDING_FILE_BYTES {
                return Err(EmbeddingInstallError::InvalidPin);
            }
            Ok(EmbeddingPinnedFile {
                role: remote.role,
                remote_path: file.path,
                byte_size: file.size,
                digest,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EmbeddingPin {
        family,
        revision: pinned.revision,
        files,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbeddingInstallError {
    #[error("embedding repository listing is invalid")]
    InvalidPin,
    #[error("embedding install does not match its pin")]
    PinMismatch,
    #[error("embedding artifact failed verification: {0}")]
    Artifact(#[from] ModelArtifactError),
    #[error("embedding install storage failed")]
    Storage,
}

/// Installed embedding models below one root: one `manifest.json` per
/// family in the family's folder, pointing at verified files.
#[derive(Debug, Clone)]
pub struct EmbeddingInstallStore {
    root: PathBuf,
}

impl EmbeddingInstallStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn manifest_path(&self, family: EmbeddingModelFamily) -> PathBuf {
        self.root.join(family.install_dir()).join(MANIFEST_FILE)
    }

    /// The recorded manifest, not yet reverified; `None` when the family was
    /// never installed. A manifest recorded for another family is refused.
    pub fn manifest(
        &self,
        family: EmbeddingModelFamily,
    ) -> Result<Option<InstalledEmbeddingManifest>, EmbeddingInstallError> {
        let path = self.manifest_path(family);
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(EmbeddingInstallError::Storage),
        };
        if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
            return Err(ModelArtifactError::InvalidManifest.into());
        }
        let bytes = std::fs::read(&path).map_err(|_| EmbeddingInstallError::Storage)?;
        let manifest: InstalledEmbeddingManifest = serde_json::from_slice(&bytes)
            .map_err(|_| EmbeddingInstallError::Artifact(ModelArtifactError::InvalidManifest))?;
        if manifest.family != family {
            return Err(ModelArtifactError::InvalidManifest.into());
        }
        Ok(Some(manifest))
    }

    /// Verifies the manifest's files and records it atomically, replacing
    /// the family's previous manifest.
    pub fn record(
        &self,
        manifest: &InstalledEmbeddingManifest,
    ) -> Result<(), EmbeddingInstallError> {
        manifest.verify()?;
        let path = self.manifest_path(manifest.family);
        let folder = path.parent().ok_or(EmbeddingInstallError::Storage)?;
        std::fs::create_dir_all(folder).map_err(|_| EmbeddingInstallError::Storage)?;
        let bytes =
            serde_json::to_vec_pretty(manifest).map_err(|_| EmbeddingInstallError::Storage)?;
        let partial = folder.join(format!("{MANIFEST_FILE}.partial"));
        let mut file = File::create(&partial).map_err(|_| EmbeddingInstallError::Storage)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| EmbeddingInstallError::Storage)?;
        drop(file);
        std::fs::rename(&partial, &path).map_err(|_| EmbeddingInstallError::Storage)
    }

    /// Every family with a recorded manifest.
    pub fn installed(&self) -> Result<Vec<InstalledEmbeddingManifest>, EmbeddingInstallError> {
        EmbeddingModelFamily::ALL
            .iter()
            .filter_map(|family| self.manifest(*family).transpose())
            .collect()
    }

    /// Removes the family's manifest, its downloaded files and any recorded
    /// file inside this root; files outside the root are left alone.
    /// Returns whether anything was installed.
    pub fn remove(&self, family: EmbeddingModelFamily) -> Result<bool, EmbeddingInstallError> {
        let manifest = self.manifest(family).ok().flatten();
        let folder = self.root.join(family.install_dir());
        let existed = manifest.is_some() || folder.exists();
        if let Some(manifest) = &manifest {
            for artifact in manifest.artifacts() {
                if artifact.path.starts_with(&self.root) && !artifact.path.starts_with(&folder) {
                    match std::fs::remove_file(&artifact.path) {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(_) => return Err(EmbeddingInstallError::Storage),
                    }
                }
            }
        }
        match std::fs::remove_dir_all(&folder) {
            Ok(()) => Ok(existed),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(existed),
            Err(_) => Err(EmbeddingInstallError::Storage),
        }
    }
}

/// The legacy v4 install (`v4-model.int8.onnx` and `v4-tokenizer.json` in
/// the legacy embedding folder) as a manifest over the files where they are;
/// `None` unless both files exist. Its revision names the model bytes.
pub fn inspect_legacy_embedding_install(
    legacy_root: &Path,
) -> Result<Option<InstalledEmbeddingManifest>, ModelArtifactError> {
    let model = legacy_root.join(LEGACY_V4_MODEL_FILE);
    let tokenizer = legacy_root.join(LEGACY_V4_TOKENIZER_FILE);
    for path in [&model, &tokenizer] {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => return Err(ModelArtifactError::InvalidManifest),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ModelArtifactError::Unreadable),
        }
    }
    let model = InstalledModelArtifact::inspect(model)?;
    let tokenizer = InstalledModelArtifact::inspect(tokenizer)?;
    let family = EmbeddingModelFamily::LettuceEmbV4;
    Ok(Some(InstalledEmbeddingManifest {
        family,
        source_revision: format!("legacy-import:{}", &model.blake3.as_str()[..16]),
        model,
        tokenizer,
        calibration: None,
        max_sequence_length: family.max_positions(),
        native_dimensions: family.native_dimensions(),
    }))
}

/// The installed family to load: the preferred one when installed, else
/// Eidos, else v4.
#[must_use]
pub fn select_embedding_family(
    preferred: Option<EmbeddingModelFamily>,
    installed: &[EmbeddingModelFamily],
) -> Option<EmbeddingModelFamily> {
    preferred
        .filter(|family| installed.contains(family))
        .or_else(|| {
            EmbeddingModelFamily::ALL
                .into_iter()
                .find(|family| installed.contains(family))
        })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use lettuce_types::{ContentHash, OperationId};

    use super::*;

    fn artifact(path: std::path::PathBuf, bytes: &[u8]) -> InstalledModelArtifact {
        InstalledModelArtifact {
            path,
            byte_size: u64::try_from(bytes.len()).expect("test size"),
            blake3: ContentHash::parse(blake3::hash(bytes).to_hex().to_string()).expect("hash"),
        }
    }

    fn write(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        std::fs::File::create(path)
            .and_then(|mut file| file.write_all(bytes))
            .expect("file");
    }

    #[test]
    fn verifies_complete_immutable_embedding_artifacts() {
        let root = std::env::temp_dir().join(format!("embedding-{}", OperationId::new()));
        let model_path = root.join("model.onnx");
        let tokenizer_path = root.join("tokenizer.json");
        write(&model_path, b"model");
        write(&tokenizer_path, b"tokenizer");
        let manifest = InstalledEmbeddingManifest {
            family: EmbeddingModelFamily::LettuceEmbV4,
            source_revision: "legacy-import:dbeeaecd".to_owned(),
            model: artifact(model_path, b"model"),
            tokenizer: artifact(tokenizer_path, b"tokenizer"),
            calibration: None,
            max_sequence_length: 128,
            native_dimensions: 768,
        };
        assert!(manifest.verify().is_ok());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_tampered_artifacts() {
        let path = std::env::temp_dir().join(format!("embedding-{}", OperationId::new()));
        std::fs::write(&path, b"tampered").expect("file");
        let manifest = InstalledEmbeddingManifest {
            family: EmbeddingModelFamily::LettuceEmbV4,
            source_revision: "revision".to_owned(),
            model: artifact(path.clone(), b"expected"),
            tokenizer: artifact(path.clone(), b"expected"),
            calibration: None,
            max_sequence_length: 128,
            native_dimensions: 768,
        };
        assert_eq!(manifest.verify(), Err(ModelArtifactError::Mismatch));
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn each_family_bounds_its_sequence_length_and_calibration() {
        let root = std::env::temp_dir().join(format!("embedding-{}", OperationId::new()));
        let model_path = root.join("model.onnx");
        let tokenizer_path = root.join("tokenizer.json");
        let calibration_path = root.join("calibration.json");
        write(&model_path, b"model");
        write(&tokenizer_path, b"tokenizer");
        write(&calibration_path, b"{}");
        let eidos = InstalledEmbeddingManifest {
            family: EmbeddingModelFamily::LettuceEidosV5,
            source_revision: "f".repeat(40),
            model: artifact(model_path.clone(), b"model"),
            tokenizer: artifact(tokenizer_path.clone(), b"tokenizer"),
            calibration: Some(artifact(calibration_path.clone(), b"{}")),
            max_sequence_length: 4096,
            native_dimensions: 768,
        };
        let verified = eidos.verify().expect("eidos at 4096");
        assert_eq!(verified.calibration_path, Some(calibration_path));
        assert_eq!(
            InstalledEmbeddingManifest {
                calibration: None,
                ..eidos.clone()
            }
            .verify(),
            Err(ModelArtifactError::InvalidManifest)
        );
        assert_eq!(
            InstalledEmbeddingManifest {
                max_sequence_length: 4097,
                ..eidos.clone()
            }
            .verify(),
            Err(ModelArtifactError::InvalidManifest)
        );
        let v4 = InstalledEmbeddingManifest {
            family: EmbeddingModelFamily::LettuceEmbV4,
            calibration: None,
            ..eidos.clone()
        };
        assert_eq!(v4.verify(), Err(ModelArtifactError::InvalidManifest));
        assert!(
            InstalledEmbeddingManifest {
                max_sequence_length: 2048,
                ..v4.clone()
            }
            .verify()
            .is_ok()
        );
        assert_eq!(
            InstalledEmbeddingManifest {
                calibration: eidos.calibration.clone(),
                max_sequence_length: 2048,
                ..v4
            }
            .verify(),
            Err(ModelArtifactError::InvalidManifest)
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn families_keep_distinct_vector_spaces_and_the_legacy_v4_label() {
        assert_eq!(EmbeddingModelFamily::LettuceEmbV4.vector_space(), "v4");
        assert_eq!(EmbeddingModelFamily::LettuceEidosV5.vector_space(), "v5");
        assert_eq!(
            serde_json::to_string(&EmbeddingModelFamily::LettuceEidosV5).expect("json"),
            "\"lettuce_eidos_v5\""
        );
    }

    fn pin_body() -> String {
        serde_json::json!({
            "sha": "F14E5DE6AB468DF6651B6505F59A7224E630EE79",
            "siblings": [
                {"rfilename": "calibration.json", "blobId": "19f91c7a0f6ef98c90d72f6a94c05275a0c42eba", "size": 2006},
                {"rfilename": "onnx/model.onnx", "size": 1046400328, "lfs": {"sha256": "d442d0f34c737e593379ec02ab38716f47af7ce6200672e5710145b1a25c151d", "size": 1046400328}},
                {"rfilename": "onnx/model_quantized.onnx", "blobId": "2581a0e0ae9f4d1dab85f417a5145d673c01ed91", "size": 262098715, "lfs": {"sha256": "7a05962e32ef57edd9a69adeea152d582d53694e5c088d549bf3f410baa5fb74", "size": 262098715}},
                {"rfilename": "tokenizer.json", "blobId": "d385841006f755137d15b0a114e80a0da59752a7", "size": 33384665, "lfs": {"sha256": "6380125d482ad297b1b147e0669c53aec88b4ef5113debda747212533ba42d59", "size": 33384665}}
            ]
        })
        .to_string()
    }

    #[test]
    fn pins_the_listed_revision_with_a_digest_for_every_file() {
        let pin = parse_embedding_pin(EmbeddingModelFamily::LettuceEidosV5, pin_body().as_bytes())
            .expect("pin");
        assert_eq!(pin.revision, "f14e5de6ab468df6651b6505f59a7224e630ee79");
        assert_eq!(
            pin.files
                .iter()
                .map(|file| (file.role, file.local_name(), file.byte_size))
                .collect::<Vec<_>>(),
            vec![
                (
                    EmbeddingArtifactRole::Model,
                    "model_quantized.onnx",
                    262_098_715
                ),
                (
                    EmbeddingArtifactRole::Tokenizer,
                    "tokenizer.json",
                    33_384_665
                ),
                (EmbeddingArtifactRole::Calibration, "calibration.json", 2006),
            ]
        );
        assert_eq!(
            pin.files[2].digest,
            EmbeddingFileDigest::GitBlobSha1("19f91c7a0f6ef98c90d72f6a94c05275a0c42eba".to_owned())
        );
        assert_eq!(
            pin.local_segments(&pin.files[0]),
            vec![
                "lettuce-eidos-768d-v5".to_owned(),
                pin.revision.clone(),
                "model_quantized.onnx".to_owned()
            ]
        );
        let without_calibration = pin_body().replace("calibration.json", "other.json");
        assert_eq!(
            parse_embedding_pin(
                EmbeddingModelFamily::LettuceEidosV5,
                without_calibration.as_bytes()
            ),
            Err(EmbeddingInstallError::InvalidPin)
        );
        let without_digest = pin_body().replace("\"blobId\":\"19f9", "\"other\":\"19f9");
        assert_eq!(
            parse_embedding_pin(
                EmbeddingModelFamily::LettuceEidosV5,
                without_digest.as_bytes()
            ),
            Err(EmbeddingInstallError::InvalidPin)
        );
    }

    #[test]
    fn store_records_replays_and_removes_installs_and_adopts_legacy_v4_files() {
        let root = std::env::temp_dir().join(format!("embedding-store-{}", OperationId::new()));
        let store = EmbeddingInstallStore::new(&root);
        assert_eq!(store.installed(), Ok(Vec::new()));
        assert_eq!(inspect_legacy_embedding_install(&root), Ok(None));
        write(&root.join("v4-model.int8.onnx"), b"legacy model");
        assert_eq!(inspect_legacy_embedding_install(&root), Ok(None));
        write(&root.join("v4-tokenizer.json"), b"legacy tokenizer");
        let legacy = inspect_legacy_embedding_install(&root)
            .expect("inspect")
            .expect("legacy install");
        assert_eq!(legacy.family, EmbeddingModelFamily::LettuceEmbV4);
        assert!(legacy.source_revision.starts_with("legacy-import:"));
        assert_eq!(legacy.max_sequence_length, 2048);
        store.record(&legacy).expect("record legacy");

        let folder = root.join("lettuce-eidos-768d-v5").join("f".repeat(40));
        for (name, bytes) in [
            ("model_quantized.onnx", b"model".as_slice()),
            ("tokenizer.json", b"tokenizer".as_slice()),
            ("calibration.json", b"{}".as_slice()),
        ] {
            write(&folder.join(name), bytes);
        }
        let eidos = InstalledEmbeddingManifest {
            family: EmbeddingModelFamily::LettuceEidosV5,
            source_revision: "f".repeat(40),
            model: artifact(folder.join("model_quantized.onnx"), b"model"),
            tokenizer: artifact(folder.join("tokenizer.json"), b"tokenizer"),
            calibration: Some(artifact(folder.join("calibration.json"), b"{}")),
            max_sequence_length: 4096,
            native_dimensions: 768,
        };
        store.record(&eidos).expect("record eidos");
        let reopened = EmbeddingInstallStore::new(&root);
        assert_eq!(
            reopened.manifest(EmbeddingModelFamily::LettuceEidosV5),
            Ok(Some(eidos))
        );
        assert_eq!(reopened.installed().expect("installed").len(), 2);
        let installed = [
            EmbeddingModelFamily::LettuceEmbV4,
            EmbeddingModelFamily::LettuceEidosV5,
        ];
        assert_eq!(
            select_embedding_family(Some(EmbeddingModelFamily::LettuceEmbV4), &installed),
            Some(EmbeddingModelFamily::LettuceEmbV4)
        );
        assert_eq!(
            select_embedding_family(None, &installed),
            Some(EmbeddingModelFamily::LettuceEidosV5)
        );
        assert_eq!(
            select_embedding_family(
                Some(EmbeddingModelFamily::LettuceEidosV5),
                &[EmbeddingModelFamily::LettuceEmbV4]
            ),
            Some(EmbeddingModelFamily::LettuceEmbV4)
        );
        assert_eq!(select_embedding_family(None, &[]), None);

        assert_eq!(
            reopened.remove(EmbeddingModelFamily::LettuceEidosV5),
            Ok(true)
        );
        assert!(!root.join("lettuce-eidos-768d-v5").exists());
        assert_eq!(
            reopened.remove(EmbeddingModelFamily::LettuceEidosV5),
            Ok(false)
        );
        assert_eq!(
            reopened.remove(EmbeddingModelFamily::LettuceEmbV4),
            Ok(true)
        );
        assert!(!root.join("v4-model.int8.onnx").exists());
        assert!(!root.join("v4-tokenizer.json").exists());
        assert_eq!(reopened.installed(), Ok(Vec::new()));
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
