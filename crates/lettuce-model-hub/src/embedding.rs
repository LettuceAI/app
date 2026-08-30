use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use lettuce_types::ContentHash;
use serde::{Deserialize, Serialize};

pub(crate) const MAX_SOURCE_REVISION_BYTES: usize = 128;
const MAX_EMBEDDING_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingModelFamily {
    LettuceEmbV4,
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
            || self.max_sequence_length > 2048
            || self.native_dimensions != 768
        {
            return Err(ModelArtifactError::InvalidManifest);
        }
        verify_artifact(&self.model)?;
        verify_artifact(&self.tokenizer)?;
        Ok(VerifiedEmbeddingArtifacts {
            family: self.family,
            source_revision: revision.to_owned(),
            model_path: self.model.path.clone(),
            tokenizer_path: self.tokenizer.path.clone(),
            max_sequence_length: self.max_sequence_length,
            native_dimensions: self.native_dimensions,
        })
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

#[cfg(test)]
mod tests {
    use std::io::Write;

    use lettuce_types::{ContentHash, OperationId};

    use super::{
        EmbeddingModelFamily, InstalledEmbeddingManifest, InstalledModelArtifact,
        ModelArtifactError,
    };

    fn artifact(path: std::path::PathBuf, bytes: &[u8]) -> InstalledModelArtifact {
        InstalledModelArtifact {
            path,
            byte_size: u64::try_from(bytes.len()).expect("test size"),
            blake3: ContentHash::parse(blake3::hash(bytes).to_hex().to_string()).expect("hash"),
        }
    }

    #[test]
    fn verifies_complete_immutable_embedding_artifacts() {
        let root = std::env::temp_dir().join(format!("embedding-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("directory");
        let model_bytes = b"model";
        let tokenizer_bytes = b"tokenizer";
        let model_path = root.join("model.onnx");
        let tokenizer_path = root.join("tokenizer.json");
        std::fs::File::create(&model_path)
            .and_then(|mut file| file.write_all(model_bytes))
            .expect("model");
        std::fs::File::create(&tokenizer_path)
            .and_then(|mut file| file.write_all(tokenizer_bytes))
            .expect("tokenizer");
        let manifest = InstalledEmbeddingManifest {
            family: EmbeddingModelFamily::LettuceEmbV4,
            source_revision: "legacy-import:dbeeaecd".to_owned(),
            model: artifact(model_path, model_bytes),
            tokenizer: artifact(tokenizer_path, tokenizer_bytes),
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
            max_sequence_length: 128,
            native_dimensions: 768,
        };
        assert_eq!(manifest.verify(), Err(ModelArtifactError::Mismatch));
        std::fs::remove_file(path).expect("cleanup");
    }
}
