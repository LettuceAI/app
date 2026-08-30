use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{InstalledModelArtifact, ModelArtifactError};

use super::embedding::{MAX_SOURCE_REVISION_BYTES, verify_artifact};

pub const COMPANION_EMOTION_MAX_SEQUENCE_LENGTH: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledCompanionEmotionManifest {
    /// Immutable upstream commit, release, or imported legacy artifact ID.
    pub source_revision: String,
    pub model: InstalledModelArtifact,
    pub tokenizer: InstalledModelArtifact,
    pub config: InstalledModelArtifact,
}

impl InstalledCompanionEmotionManifest {
    pub fn verify(&self) -> Result<VerifiedCompanionEmotionArtifacts, ModelArtifactError> {
        let revision = self.source_revision.trim();
        if revision.is_empty() || revision.len() > MAX_SOURCE_REVISION_BYTES {
            return Err(ModelArtifactError::InvalidManifest);
        }
        verify_artifact(&self.model)?;
        verify_artifact(&self.tokenizer)?;
        verify_artifact(&self.config)?;
        Ok(VerifiedCompanionEmotionArtifacts {
            source_revision: revision.to_owned(),
            model_path: self.model.path.clone(),
            tokenizer_path: self.tokenizer.path.clone(),
            config_path: self.config.path.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCompanionEmotionArtifacts {
    pub source_revision: String,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub config_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use lettuce_types::{ContentHash, OperationId};

    use super::*;

    fn artifact(path: PathBuf, bytes: &[u8]) -> InstalledModelArtifact {
        InstalledModelArtifact {
            path,
            byte_size: u64::try_from(bytes.len()).expect("test size"),
            blake3: ContentHash::parse(blake3::hash(bytes).to_hex().to_string()).expect("hash"),
        }
    }

    #[test]
    fn verifies_all_three_immutable_companion_emotion_artifacts() {
        let root = std::env::temp_dir().join(format!("companion-emotion-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("directory");
        let paths = [
            (root.join("model.int8.onnx"), b"model".as_slice()),
            (root.join("tokenizer.json"), b"tokenizer".as_slice()),
            (root.join("config.json"), b"config".as_slice()),
        ];
        for (path, bytes) in &paths {
            std::fs::File::create(path)
                .and_then(|mut file| file.write_all(bytes))
                .expect("artifact");
        }
        let manifest = InstalledCompanionEmotionManifest {
            source_revision: "SamLowe/roberta-base-go_emotions-onnx".into(),
            model: artifact(paths[0].0.clone(), paths[0].1),
            tokenizer: artifact(paths[1].0.clone(), paths[1].1),
            config: artifact(paths[2].0.clone(), paths[2].1),
        };
        let verified = manifest.verify().expect("verified");
        assert_eq!(verified.model_path, paths[0].0);
        assert_eq!(verified.tokenizer_path, paths[1].0);
        assert_eq!(verified.config_path, paths[2].0);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_a_tampered_config_artifact() {
        let path = std::env::temp_dir().join(format!("companion-emotion-{}", OperationId::new()));
        std::fs::write(&path, b"tampered").expect("file");
        let expected = artifact(path.clone(), b"expected");
        let manifest = InstalledCompanionEmotionManifest {
            source_revision: "revision".into(),
            model: InstalledModelArtifact::inspect(path.clone()).expect("model"),
            tokenizer: InstalledModelArtifact::inspect(path.clone()).expect("tokenizer"),
            config: expected,
        };
        assert_eq!(manifest.verify(), Err(ModelArtifactError::Mismatch));
        std::fs::remove_file(path).expect("cleanup");
    }
}
