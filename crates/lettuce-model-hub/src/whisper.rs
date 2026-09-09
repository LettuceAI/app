use std::{
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};

use lettuce_platform::{ConfinedInstallStore, InstallPreparation, ObjectKey, ResumableInstall};
use lettuce_types::{ContentHash, TimestampMillis};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::InstalledModelArtifact;

pub const MAX_WHISPER_MODEL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_MODEL_ID_SCALARS: usize = 128;
const MAX_SOURCE_REVISION_BYTES: usize = 128;
const SHA256_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteWhisperModel {
    pub model_id: String,
    pub filename: String,
    pub source_revision: String,
    pub byte_size: u64,
    pub sha256: String,
    pub english_only: bool,
    pub quantized: bool,
    pub recommended: bool,
    pub recommended_for_mobile: bool,
    pub recommended_for_desktop: bool,
}

impl RemoteWhisperModel {
    pub fn pinned(
        filename: impl Into<String>,
        source_revision: impl Into<String>,
        byte_size: u64,
        sha256: impl Into<String>,
    ) -> Result<Self, WhisperModelError> {
        let filename = filename.into();
        let model_id = model_id_from_filename(&filename)?;
        let source_revision = source_revision.into();
        let sha256 = sha256.into().to_ascii_lowercase();
        let model = Self {
            english_only: model_id.contains(".en"),
            quantized: model_id.contains("-q"),
            recommended: is_recommended(&model_id),
            recommended_for_mobile: is_recommended_for_mobile(&model_id),
            recommended_for_desktop: is_recommended_for_desktop(&model_id),
            model_id,
            filename,
            source_revision: source_revision.to_ascii_lowercase(),
            byte_size,
            sha256,
        };
        model.validate()?;
        Ok(model)
    }

    pub fn validate(&self) -> Result<(), WhisperModelError> {
        if model_id_from_filename(&self.filename)? != self.model_id
            || self.source_revision.len() != 40
            || !self
                .source_revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.source_revision != self.source_revision.to_ascii_lowercase()
            || self.byte_size == 0
            || self.byte_size > MAX_WHISPER_MODEL_BYTES
            || self.sha256.len() != SHA256_HEX_LENGTH
            || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || self.sha256 != self.sha256.to_ascii_lowercase()
            || self.english_only != self.model_id.contains(".en")
            || self.quantized != self.model_id.contains("-q")
            || self.recommended != is_recommended(&self.model_id)
            || self.recommended_for_mobile != is_recommended_for_mobile(&self.model_id)
            || self.recommended_for_desktop != is_recommended_for_desktop(&self.model_id)
        {
            return Err(WhisperModelError::InvalidManifest);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct WhisperInstallStore {
    inner: ConfinedInstallStore,
}

#[derive(Debug)]
pub enum WhisperInstallPreparation {
    Installed(InstalledWhisperManifest),
    Download(WhisperDownloadSession),
}

#[derive(Debug)]
pub struct WhisperDownloadSession {
    inner: ResumableInstall,
    remote: RemoteWhisperModel,
    admitted_at: TimestampMillis,
}

impl WhisperInstallStore {
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self, WhisperModelError> {
        ConfinedInstallStore::open(root_path)
            .map(|inner| Self { inner })
            .map_err(map_platform_error)
    }

    pub fn prepare(
        &self,
        remote: RemoteWhisperModel,
        admitted_at: TimestampMillis,
    ) -> Result<WhisperInstallPreparation, WhisperModelError> {
        remote.validate()?;
        let partial_name = partial_name(&remote);
        let partial = ObjectKey::from_segments(["downloads", partial_name.as_str()])
            .map_err(map_platform_error)?;
        let target = ObjectKey::from_segments([remote.model_id.as_str(), remote.filename.as_str()])
            .map_err(map_platform_error)?;
        match self
            .inner
            .prepare(partial, target, remote.byte_size)
            .map_err(map_platform_error)?
        {
            InstallPreparation::Installed(mut installed) => {
                if installed.len() != remote.byte_size {
                    return Err(WhisperModelError::Mismatch);
                }
                installed.rewind().map_err(map_platform_error)?;
                verify_sha256(&mut installed, remote.byte_size, &remote.sha256)?;
                Ok(WhisperInstallPreparation::Installed(installed_manifest(
                    installed.native_path(),
                    &remote,
                    admitted_at,
                )?))
            }
            InstallPreparation::Resume(inner) => Ok(WhisperInstallPreparation::Download(
                WhisperDownloadSession {
                    inner,
                    remote,
                    admitted_at,
                },
            )),
        }
    }

    pub fn remove_managed(
        &self,
        manifest: &InstalledWhisperManifest,
    ) -> Result<bool, WhisperModelError> {
        self.validate_managed(manifest)?;
        let target = managed_target(manifest)?;
        self.inner
            .remove_installed(&target, &manifest.model.path)
            .map_err(map_platform_error)
    }

    pub fn validate_managed(
        &self,
        manifest: &InstalledWhisperManifest,
    ) -> Result<(), WhisperModelError> {
        manifest.validate()?;
        if manifest.source_revision.len() != 40
            || !manifest
                .source_revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(WhisperModelError::OutsideSource);
        }
        let target = managed_target(manifest)?;
        if !self
            .inner
            .owns_installed_path(&target, &manifest.model.path)
        {
            return Err(WhisperModelError::OutsideSource);
        }
        Ok(())
    }
}

fn managed_target(manifest: &InstalledWhisperManifest) -> Result<ObjectKey, WhisperModelError> {
    let filename = whisper_filename(&manifest.model_id);
    ObjectKey::from_segments([manifest.model_id.as_str(), filename.as_str()])
        .map_err(map_platform_error)
}

impl WhisperDownloadSession {
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.inner.offset()
    }

    pub fn restart(&mut self) -> Result<(), WhisperModelError> {
        self.inner.restart().map_err(map_platform_error)
    }

    pub fn append(&mut self, bytes: &[u8]) -> Result<u64, WhisperModelError> {
        self.inner.append(bytes).map_err(map_platform_error)
    }

    pub fn finish(mut self) -> Result<InstalledWhisperManifest, WhisperModelError> {
        if self.inner.offset() != self.remote.byte_size {
            return Err(WhisperModelError::Mismatch);
        }
        self.inner.sync().map_err(map_platform_error)?;
        self.inner.rewind().map_err(map_platform_error)?;
        verify_sha256(&mut self.inner, self.remote.byte_size, &self.remote.sha256)?;
        let path = self.inner.commit().map_err(map_platform_error)?;
        installed_manifest(&path, &self.remote, self.admitted_at)
    }
}

fn partial_name(remote: &RemoteWhisperModel) -> String {
    let mut hash = blake3::Hasher::new();
    for value in [
        remote.model_id.as_bytes(),
        remote.source_revision.as_bytes(),
        remote.sha256.as_bytes(),
    ] {
        hash.update(value);
        hash.update(&[0]);
    }
    hash.update(&remote.byte_size.to_le_bytes());
    format!("{}.part", hash.finalize().to_hex())
}

fn verify_sha256(
    file: &mut impl Read,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), WhisperModelError> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| WhisperModelError::Unreadable)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| WhisperModelError::Mismatch)?)
            .ok_or(WhisperModelError::Mismatch)?;
        if total > expected_size {
            return Err(WhisperModelError::Mismatch);
        }
        hasher.update(&buffer[..read]);
    }
    if total != expected_size || format!("{:x}", hasher.finalize()) != expected_sha256 {
        return Err(WhisperModelError::Mismatch);
    }
    Ok(())
}

fn installed_manifest(
    path: &Path,
    remote: &RemoteWhisperModel,
    admitted_at: TimestampMillis,
) -> Result<InstalledWhisperManifest, WhisperModelError> {
    let blake3 = hash_file(path, remote.byte_size)?;
    Ok(InstalledWhisperManifest {
        model_id: remote.model_id.clone(),
        source_revision: remote.source_revision.clone(),
        model: InstalledModelArtifact {
            path: path.to_path_buf(),
            byte_size: remote.byte_size,
            blake3,
        },
        english_only: remote.english_only,
        quantized: remote.quantized,
        admitted_at,
    })
}

fn map_platform_error(error: lettuce_platform::PlatformError) -> WhisperModelError {
    match error {
        lettuce_platform::PlatformError::SymlinkEscape => WhisperModelError::Symlink,
        lettuce_platform::PlatformError::Denied | lettuce_platform::PlatformError::InvalidKey => {
            WhisperModelError::OutsideSource
        }
        lettuce_platform::PlatformError::LimitExceeded => WhisperModelError::Mismatch,
        _ => WhisperModelError::Unreadable,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledWhisperManifest {
    pub model_id: String,
    pub source_revision: String,
    pub model: InstalledModelArtifact,
    pub english_only: bool,
    pub quantized: bool,
    pub admitted_at: TimestampMillis,
}

impl InstalledWhisperManifest {
    pub fn inspect_legacy(
        legacy_models_root: &Path,
        model_path: &Path,
        admitted_at: TimestampMillis,
    ) -> Result<Self, WhisperModelError> {
        validate_confined_regular_file(legacy_models_root, model_path)?;
        let filename = model_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(WhisperModelError::InvalidManifest)?;
        let model_id = model_id_from_filename(filename)?;
        let metadata = std::fs::metadata(model_path).map_err(|_| WhisperModelError::Unreadable)?;
        if metadata.len() == 0 || metadata.len() > MAX_WHISPER_MODEL_BYTES {
            return Err(WhisperModelError::InvalidManifest);
        }
        let hash = hash_file(model_path, metadata.len())?;
        Ok(Self {
            english_only: model_id.contains(".en"),
            quantized: model_id.contains("-q"),
            source_revision: format!("legacy-import:{hash}"),
            model_id,
            model: InstalledModelArtifact {
                path: model_path.to_path_buf(),
                byte_size: metadata.len(),
                blake3: hash,
            },
            admitted_at,
        })
    }

    pub fn verify(&self) -> Result<VerifiedWhisperArtifacts, WhisperModelError> {
        self.validate()?;
        let metadata =
            std::fs::metadata(&self.model.path).map_err(|_| WhisperModelError::Missing)?;
        if !metadata.is_file() || metadata.len() != self.model.byte_size {
            return Err(WhisperModelError::Mismatch);
        }
        if hash_file(&self.model.path, self.model.byte_size)? != self.model.blake3 {
            return Err(WhisperModelError::Mismatch);
        }
        Ok(VerifiedWhisperArtifacts {
            model_id: self.model_id.clone(),
            source_revision: self.source_revision.clone(),
            model_path: self.model.path.clone(),
            byte_size: self.model.byte_size,
            blake3: self.model.blake3.clone(),
            english_only: self.english_only,
            quantized: self.quantized,
        })
    }

    pub fn validate(&self) -> Result<(), WhisperModelError> {
        validate_model_id(&self.model_id)?;
        if self.source_revision.trim() != self.source_revision
            || self.source_revision.is_empty()
            || self.source_revision.len() > MAX_SOURCE_REVISION_BYTES
            || self.source_revision.chars().any(char::is_control)
            || self.model.byte_size == 0
            || self.model.byte_size > MAX_WHISPER_MODEL_BYTES
        {
            return Err(WhisperModelError::InvalidManifest);
        }
        let filename = self
            .model
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(WhisperModelError::InvalidManifest)?;
        if model_id_from_filename(filename)? != self.model_id
            || self.english_only != self.model_id.contains(".en")
            || self.quantized != self.model_id.contains("-q")
        {
            return Err(WhisperModelError::InvalidManifest);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedWhisperArtifacts {
    pub model_id: String,
    pub source_revision: String,
    pub model_path: PathBuf,
    pub byte_size: u64,
    pub blake3: ContentHash,
    pub english_only: bool,
    pub quantized: bool,
}

pub trait WhisperModelRepository: Send + Sync {
    fn admit_whisper_model(
        &self,
        manifest: InstalledWhisperManifest,
    ) -> Result<InstalledWhisperManifest, WhisperModelRepositoryError>;
    fn get_whisper_model(
        &self,
        model_id: &str,
    ) -> Result<Option<InstalledWhisperManifest>, WhisperModelRepositoryError>;
    fn list_whisper_models(
        &self,
    ) -> Result<Vec<InstalledWhisperManifest>, WhisperModelRepositoryError>;
    fn remove_whisper_model(
        &self,
        expected: &InstalledWhisperManifest,
    ) -> Result<bool, WhisperModelRepositoryError>;
}

pub fn select_default_whisper_model(
    models: &[InstalledWhisperManifest],
) -> Result<Option<InstalledWhisperManifest>, WhisperModelError> {
    for model in models {
        model.validate()?;
    }
    let mut models = models.to_vec();
    models.sort_by(|left, right| {
        whisper_filename(&left.model_id).cmp(&whisper_filename(&right.model_id))
    });
    Ok(models.into_iter().next())
}

pub fn inspect_legacy_whisper_models(
    legacy_models_root: &Path,
    admitted_at: TimestampMillis,
) -> Result<Vec<InstalledWhisperManifest>, WhisperModelError> {
    let mut manifests = Vec::new();
    let folders =
        std::fs::read_dir(legacy_models_root).map_err(|_| WhisperModelError::Unreadable)?;
    for (folder_index, folder) in folders.enumerate() {
        if folder_index >= 256 {
            return Err(WhisperModelError::LimitExceeded);
        }
        let folder = folder.map_err(|_| WhisperModelError::Unreadable)?;
        let metadata = folder
            .file_type()
            .map_err(|_| WhisperModelError::Unreadable)?;
        if metadata.is_symlink() {
            return Err(WhisperModelError::Symlink);
        }
        if !metadata.is_dir() {
            continue;
        }
        let files = std::fs::read_dir(folder.path()).map_err(|_| WhisperModelError::Unreadable)?;
        for (file_index, file) in files.enumerate() {
            if file_index >= 16 || manifests.len() >= 256 {
                return Err(WhisperModelError::LimitExceeded);
            }
            let file = file.map_err(|_| WhisperModelError::Unreadable)?;
            let metadata = file
                .file_type()
                .map_err(|_| WhisperModelError::Unreadable)?;
            if metadata.is_symlink() {
                return Err(WhisperModelError::Symlink);
            }
            if !metadata.is_file() {
                continue;
            }
            let Some(filename) = file.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if model_id_from_filename(&filename).is_err() {
                continue;
            }
            manifests.push(InstalledWhisperManifest::inspect_legacy(
                legacy_models_root,
                &file.path(),
                admitted_at,
            )?);
        }
    }
    manifests.sort_by(|left, right| {
        whisper_filename(&left.model_id).cmp(&whisper_filename(&right.model_id))
    });
    Ok(manifests)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WhisperModelError {
    #[error("Whisper model manifest is invalid")]
    InvalidManifest,
    #[error("Whisper model is outside the admitted source root")]
    OutsideSource,
    #[error("Whisper model source contains a symbolic link")]
    Symlink,
    #[error("Whisper model is missing")]
    Missing,
    #[error("Whisper model cannot be read")]
    Unreadable,
    #[error("Whisper model does not match its manifest")]
    Mismatch,
    #[error("Whisper model discovery exceeds its bounded inventory")]
    LimitExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WhisperModelRepositoryError {
    #[error("Whisper model was not found")]
    NotFound,
    #[error("Whisper model conflicts with durable state")]
    Conflict,
    #[error("Whisper model data are invalid")]
    InvalidData,
    #[error("Whisper model storage failed")]
    Storage,
}

fn validate_confined_regular_file(root: &Path, path: &Path) -> Result<(), WhisperModelError> {
    if !root.is_absolute() || !path.is_absolute() {
        return Err(WhisperModelError::OutsideSource);
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| WhisperModelError::OutsideSource)?;
    if relative.components().count() != 2
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(WhisperModelError::OutsideSource);
    }
    if std::fs::symlink_metadata(root)
        .map_err(|_| WhisperModelError::Unreadable)?
        .file_type()
        .is_symlink()
    {
        return Err(WhisperModelError::Symlink);
    }
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(WhisperModelError::OutsideSource);
        };
        current.push(component);
        let metadata =
            std::fs::symlink_metadata(&current).map_err(|_| WhisperModelError::Unreadable)?;
        if metadata.file_type().is_symlink() {
            return Err(WhisperModelError::Symlink);
        }
    }
    if !std::fs::metadata(path)
        .map_err(|_| WhisperModelError::Unreadable)?
        .is_file()
    {
        return Err(WhisperModelError::InvalidManifest);
    }
    Ok(())
}

fn model_id_from_filename(filename: &str) -> Result<String, WhisperModelError> {
    let lower = filename.to_ascii_lowercase();
    if filename != lower
        || !lower.starts_with("ggml-")
        || !lower.ends_with(".bin")
        || lower.ends_with(".tmp")
        || lower.contains("encoder")
        || lower.contains(".mlmodelc")
    {
        return Err(WhisperModelError::InvalidManifest);
    }
    let model_id = filename
        .strip_prefix("ggml-")
        .and_then(|value| value.strip_suffix(".bin"))
        .ok_or(WhisperModelError::InvalidManifest)?
        .to_owned();
    validate_model_id(&model_id)?;
    Ok(model_id)
}

fn validate_model_id(model_id: &str) -> Result<(), WhisperModelError> {
    if model_id.trim() != model_id
        || model_id.is_empty()
        || model_id.chars().count() > MAX_MODEL_ID_SCALARS
        || !model_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(WhisperModelError::InvalidManifest);
    }
    Ok(())
}

fn whisper_filename(model_id: &str) -> String {
    format!("ggml-{model_id}.bin")
}

fn is_recommended(model_id: &str) -> bool {
    matches!(
        model_id,
        "base.en"
            | "base"
            | "small.en"
            | "small"
            | "medium.en-q5_0"
            | "medium-q5_0"
            | "large-v3-turbo-q5_0"
    )
}

fn is_recommended_for_mobile(model_id: &str) -> bool {
    matches!(
        model_id,
        "tiny.en"
            | "tiny"
            | "base.en-q5_1"
            | "base-q5_1"
            | "base.en"
            | "base"
            | "small.en-q5_1"
            | "small-q5_1"
    )
}

fn is_recommended_for_desktop(model_id: &str) -> bool {
    matches!(
        model_id,
        "small.en"
            | "small"
            | "medium.en-q5_0"
            | "medium-q5_0"
            | "medium.en"
            | "medium"
            | "large-v3-turbo-q5_0"
            | "large-v3-turbo"
    )
}

fn hash_file(path: &Path, expected_size: u64) -> Result<ContentHash, WhisperModelError> {
    let mut file = File::open(path).map_err(|_| WhisperModelError::Unreadable)?;
    let mut hasher = blake3::Hasher::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| WhisperModelError::Unreadable)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| WhisperModelError::Unreadable)?)
            .ok_or(WhisperModelError::InvalidManifest)?;
        if total > expected_size || total > MAX_WHISPER_MODEL_BYTES {
            return Err(WhisperModelError::Mismatch);
        }
        hasher.update(&buffer[..read]);
    }
    if total != expected_size {
        return Err(WhisperModelError::Mismatch);
    }
    ContentHash::parse(hasher.finalize().to_hex().to_string())
        .map_err(|_| WhisperModelError::InvalidManifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_types::OperationId;

    #[test]
    fn pins_remote_models_and_preserves_legacy_recommendations() {
        let model = RemoteWhisperModel::pinned(
            "ggml-small.en-q5_1.bin",
            "ab".repeat(20),
            42,
            "cd".repeat(32),
        )
        .expect("remote model");
        assert_eq!(model.model_id, "small.en-q5_1");
        assert!(model.english_only);
        assert!(model.quantized);
        assert!(model.recommended_for_mobile);
        assert!(!model.recommended_for_desktop);
        assert!(RemoteWhisperModel::pinned("ggml-base.bin", "main", 42, "cd".repeat(32)).is_err());
    }

    #[test]
    fn resumes_verified_download_and_replays_installed_manifest() {
        let root = std::env::temp_dir().join(format!("whisper-install-{}", OperationId::new()));
        let bytes = b"verified whisper bytes";
        let remote = RemoteWhisperModel::pinned(
            "ggml-base.bin",
            "ab".repeat(20),
            bytes.len() as u64,
            format!("{:x}", Sha256::digest(bytes)),
        )
        .expect("remote model");
        let store = WhisperInstallStore::open(&root).expect("install store");
        let WhisperInstallPreparation::Download(mut first) = store
            .prepare(remote.clone(), TimestampMillis::new(10))
            .expect("first preparation")
        else {
            panic!("expected download");
        };
        first.append(&bytes[..7]).expect("partial write");
        drop(first);

        let store = WhisperInstallStore::open(&root).expect("reopened store");
        let WhisperInstallPreparation::Download(mut resumed) = store
            .prepare(remote.clone(), TimestampMillis::new(10))
            .expect("resumed preparation")
        else {
            panic!("expected resumed download");
        };
        assert_eq!(resumed.offset(), 7);
        resumed.append(&bytes[7..]).expect("remaining write");
        let manifest = resumed.finish().expect("verified install");
        assert_eq!(manifest.source_revision, remote.source_revision);
        manifest.verify().expect("installed manifest");

        let WhisperInstallPreparation::Installed(replayed) = store
            .prepare(remote, TimestampMillis::new(10))
            .expect("replayed preparation")
        else {
            panic!("expected installed model");
        };
        assert_eq!(replayed, manifest);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn keeps_bad_complete_download_out_of_the_installed_directory() {
        let root = std::env::temp_dir().join(format!("whisper-install-{}", OperationId::new()));
        let remote = RemoteWhisperModel::pinned(
            "ggml-base.bin",
            "ab".repeat(20),
            3,
            format!("{:x}", Sha256::digest(b"good")),
        )
        .expect("remote model");
        let store = WhisperInstallStore::open(&root).expect("install store");
        let WhisperInstallPreparation::Download(mut download) = store
            .prepare(remote, TimestampMillis::new(10))
            .expect("preparation")
        else {
            panic!("expected download");
        };
        download.append(b"bad").expect("download bytes");
        assert_eq!(download.finish(), Err(WhisperModelError::Mismatch));
        assert!(!root.join("base/ggml-base.bin").exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn admits_and_verifies_a_confined_legacy_model() {
        let root = std::env::temp_dir().join(format!("whisper-models-{}", OperationId::new()));
        let folder = root.join("small.en-q5_1");
        std::fs::create_dir_all(&folder).expect("model directory");
        let path = folder.join("ggml-small.en-q5_1.bin");
        std::fs::write(&path, b"verified whisper model").expect("model file");
        let manifest =
            InstalledWhisperManifest::inspect_legacy(&root, &path, TimestampMillis::new(10))
                .expect("manifest");
        assert_eq!(manifest.model_id, "small.en-q5_1");
        assert!(manifest.english_only);
        assert!(manifest.quantized);
        assert_eq!(manifest.verify().expect("verified").model_path, path);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_tampering_and_files_outside_the_root() {
        let root = std::env::temp_dir().join(format!("whisper-models-{}", OperationId::new()));
        let folder = root.join("base");
        std::fs::create_dir_all(&folder).expect("model directory");
        let path = folder.join("ggml-base.bin");
        std::fs::write(&path, b"original").expect("model file");
        let manifest =
            InstalledWhisperManifest::inspect_legacy(&root, &path, TimestampMillis::new(10))
                .expect("manifest");
        std::fs::write(&path, b"changed").expect("tamper model");
        assert_eq!(manifest.verify(), Err(WhisperModelError::Mismatch));
        assert_eq!(
            InstalledWhisperManifest::inspect_legacy(
                &root,
                &root.join("ggml-base.bin"),
                TimestampMillis::new(10)
            ),
            Err(WhisperModelError::OutsideSource)
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
