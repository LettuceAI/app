use std::{
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};

use lettuce_types::{ContentHash, TimestampMillis};
use serde::{Deserialize, Serialize};

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
        if source_revision.len() != 40
            || !source_revision.bytes().all(|byte| byte.is_ascii_hexdigit())
            || byte_size == 0
            || byte_size > MAX_WHISPER_MODEL_BYTES
            || sha256.len() != SHA256_HEX_LENGTH
            || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(WhisperModelError::InvalidManifest);
        }
        Ok(Self {
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
        })
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
