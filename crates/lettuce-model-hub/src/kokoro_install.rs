use std::{io::Read, path::Path};

use lettuce_platform::{ConfinedInstallStore, InstallPreparation, ObjectKey, ResumableInstall};
use sha2::{Digest, Sha256};

use crate::{InstalledModelArtifact, KokoroModelVariant, ModelArtifactError};

pub const KOKORO_REPOSITORY: &str = "onnx-community/Kokoro-82M-v1.0-ONNX";
pub const KOKORO_SOURCE_REVISION: &str = "1939ad2a8e416c0acfeecc08a694d14ef25f2231";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KokoroArtifactRole {
    Config,
    Tokenizer,
    TokenizerConfig,
    Model,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteKokoroArtifact {
    pub role: KokoroArtifactRole,
    pub remote_path: &'static str,
    pub local_segments: &'static [&'static str],
    pub byte_size: u64,
    pub sha256: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteKokoroModel {
    pub variant: KokoroModelVariant,
    pub source_revision: &'static str,
    pub artifacts: Vec<RemoteKokoroArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledKokoroArtifact {
    pub role: KokoroArtifactRole,
    pub artifact: InstalledModelArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledKokoroModel {
    pub variant: KokoroModelVariant,
    pub source_revision: String,
    pub artifacts: Vec<InstalledKokoroArtifact>,
}

impl RemoteKokoroModel {
    pub fn validate(&self) -> Result<(), KokoroInstallError> {
        let expected = pinned_kokoro_model(self.variant);
        if self.source_revision.len() != 40
            || !self
                .source_revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.source_revision != self.source_revision.to_ascii_lowercase()
            || self.artifacts.len() != expected.artifacts.len()
            || self
                .artifacts
                .iter()
                .zip(expected.artifacts)
                .any(|(actual, expected)| {
                    actual.role != expected.role
                        || actual.remote_path != expected.remote_path
                        || actual.local_segments != expected.local_segments
                        || validate_artifact(actual).is_err()
                })
        {
            return Err(KokoroInstallError::InvalidManifest);
        }
        Ok(())
    }

    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.artifacts
            .iter()
            .map(|artifact| artifact.byte_size)
            .sum()
    }
}

#[must_use]
pub fn pinned_kokoro_model(variant: KokoroModelVariant) -> RemoteKokoroModel {
    let model = match variant {
        KokoroModelVariant::Fp32 => RemoteKokoroArtifact {
            role: KokoroArtifactRole::Model,
            remote_path: "onnx/model.onnx",
            local_segments: &["onnx", "model.onnx"],
            byte_size: 325_532_232,
            sha256: "8fbea51ea711f2af382e88c833d9e288c6dc82ce5e98421ea61c058ce21a34cb",
        },
        KokoroModelVariant::Fp16 => RemoteKokoroArtifact {
            role: KokoroArtifactRole::Model,
            remote_path: "onnx/model_fp16.onnx",
            local_segments: &["onnx", "model_fp16.onnx"],
            byte_size: 163_234_740,
            sha256: "ba4527a874b42b21e35f468c10d326fdff3c7fc8cac1f85e9eb6c0dfc35c334a",
        },
        KokoroModelVariant::Int8 => RemoteKokoroArtifact {
            role: KokoroArtifactRole::Model,
            remote_path: "onnx/model_quantized.onnx",
            local_segments: &["onnx", "model_quantized.onnx"],
            byte_size: 92_361_116,
            sha256: "fbae9257e1e05ffc727e951ef9b9c98418e6d79f1c9b6b13bd59f5c9028a1478",
        },
    };
    RemoteKokoroModel {
        variant,
        source_revision: KOKORO_SOURCE_REVISION,
        artifacts: vec![
            RemoteKokoroArtifact {
                role: KokoroArtifactRole::Config,
                remote_path: "config.json",
                local_segments: &["config.json"],
                byte_size: 44,
                sha256: "df34b4f930b23447cd4dc410fabfb42eb3f24e803e6c3f97d618fb359380a36f",
            },
            RemoteKokoroArtifact {
                role: KokoroArtifactRole::Tokenizer,
                remote_path: "tokenizer.json",
                local_segments: &["tokenizer.json"],
                byte_size: 3_497,
                sha256: "77a02c8e164413299b4b4c403b14f8e0e1c1b727db4d46a09d6327b861060a34",
            },
            RemoteKokoroArtifact {
                role: KokoroArtifactRole::TokenizerConfig,
                remote_path: "tokenizer_config.json",
                local_segments: &["tokenizer_config.json"],
                byte_size: 113,
                sha256: "be1cb066d6ef6b074b3f15e6a6dd21ac88ff3cdaedf325f0aaed686c70f75d20",
            },
            model,
        ],
    }
}

#[derive(Debug)]
pub struct KokoroInstallStore {
    inner: ConfinedInstallStore,
}

#[derive(Debug)]
pub enum KokoroArtifactPreparation {
    Installed(InstalledModelArtifact),
    Download(KokoroDownloadSession),
}

#[derive(Debug)]
pub struct KokoroDownloadSession {
    inner: ResumableInstall,
    remote: RemoteKokoroArtifact,
}

impl KokoroInstallStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, KokoroInstallError> {
        Ok(Self {
            inner: ConfinedInstallStore::open(root).map_err(KokoroInstallError::Platform)?,
        })
    }

    pub fn prepare(
        &self,
        source_revision: &str,
        remote: RemoteKokoroArtifact,
    ) -> Result<KokoroArtifactPreparation, KokoroInstallError> {
        if source_revision.len() != 40
            || !source_revision.bytes().all(|byte| byte.is_ascii_hexdigit())
            || source_revision != source_revision.to_ascii_lowercase()
        {
            return Err(KokoroInstallError::InvalidManifest);
        }
        validate_artifact(&remote)?;
        let partial = ObjectKey::from_segments([
            "downloads",
            partial_name(source_revision, &remote).as_str(),
        ])
        .map_err(KokoroInstallError::Platform)?;
        let target = ObjectKey::from_segments(remote.local_segments)
            .map_err(KokoroInstallError::Platform)?;
        match self
            .inner
            .prepare(partial, target, remote.byte_size)
            .map_err(KokoroInstallError::Platform)?
        {
            InstallPreparation::Installed(mut file) => {
                verify(&mut file, &remote)?;
                let artifact = InstalledModelArtifact::inspect(file.native_path().to_path_buf())
                    .map_err(map_artifact_error)?;
                Ok(KokoroArtifactPreparation::Installed(artifact))
            }
            InstallPreparation::Resume(inner) => {
                Ok(KokoroArtifactPreparation::Download(KokoroDownloadSession {
                    inner,
                    remote,
                }))
            }
        }
    }

    pub fn installed(
        &self,
        model: &RemoteKokoroModel,
    ) -> Result<Option<InstalledKokoroModel>, KokoroInstallError> {
        model.validate()?;
        let mut artifacts = Vec::with_capacity(model.artifacts.len());
        for remote in &model.artifacts {
            let target = ObjectKey::from_segments(remote.local_segments)
                .map_err(KokoroInstallError::Platform)?;
            let Some(mut file) = self
                .inner
                .inspect(&target)
                .map_err(KokoroInstallError::Platform)?
            else {
                return Ok(None);
            };
            verify(&mut file, remote)?;
            let artifact = InstalledModelArtifact::inspect(file.native_path().to_path_buf())
                .map_err(map_artifact_error)?;
            artifacts.push(InstalledKokoroArtifact {
                role: remote.role,
                artifact,
            });
        }
        Ok(Some(InstalledKokoroModel {
            variant: model.variant,
            source_revision: model.source_revision.to_owned(),
            artifacts,
        }))
    }
}

impl KokoroDownloadSession {
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.inner.offset()
    }

    pub fn restart(&mut self) -> Result<(), KokoroInstallError> {
        self.inner.restart().map_err(KokoroInstallError::Platform)
    }

    pub fn append(&mut self, bytes: &[u8]) -> Result<u64, KokoroInstallError> {
        self.inner
            .append(bytes)
            .map_err(KokoroInstallError::Platform)
    }

    pub fn finish(mut self) -> Result<InstalledModelArtifact, KokoroInstallError> {
        if self.inner.offset() != self.remote.byte_size {
            return Err(KokoroInstallError::Mismatch);
        }
        self.inner.sync().map_err(KokoroInstallError::Platform)?;
        self.inner.rewind().map_err(KokoroInstallError::Platform)?;
        verify(&mut self.inner, &self.remote)?;
        let path = self.inner.commit().map_err(KokoroInstallError::Platform)?;
        InstalledModelArtifact::inspect(path).map_err(map_artifact_error)
    }
}

fn validate_artifact(remote: &RemoteKokoroArtifact) -> Result<(), KokoroInstallError> {
    if remote.byte_size == 0
        || remote.byte_size > 1024 * 1024 * 1024
        || remote.sha256.len() != 64
        || !remote.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        || remote.sha256 != remote.sha256.to_ascii_lowercase()
    {
        return Err(KokoroInstallError::InvalidManifest);
    }
    Ok(())
}

fn partial_name(source_revision: &str, remote: &RemoteKokoroArtifact) -> String {
    let mut hash = blake3::Hasher::new();
    hash.update(source_revision.as_bytes());
    hash.update(&[0]);
    hash.update(remote.remote_path.as_bytes());
    hash.update(&[0]);
    hash.update(remote.sha256.as_bytes());
    hash.update(&remote.byte_size.to_le_bytes());
    format!("{}.part", hash.finalize().to_hex())
}

fn verify(file: &mut impl Read, remote: &RemoteKokoroArtifact) -> Result<(), KokoroInstallError> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| KokoroInstallError::Unreadable)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| KokoroInstallError::Mismatch)?)
            .ok_or(KokoroInstallError::Mismatch)?;
        if total > remote.byte_size {
            return Err(KokoroInstallError::Mismatch);
        }
        hasher.update(&buffer[..read]);
    }
    if total != remote.byte_size || format!("{:x}", hasher.finalize()) != remote.sha256 {
        return Err(KokoroInstallError::Mismatch);
    }
    Ok(())
}

fn map_artifact_error(error: ModelArtifactError) -> KokoroInstallError {
    match error {
        ModelArtifactError::Missing | ModelArtifactError::Unreadable => {
            KokoroInstallError::Unreadable
        }
        ModelArtifactError::InvalidManifest | ModelArtifactError::Mismatch => {
            KokoroInstallError::InvalidArtifact
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KokoroInstallError {
    #[error("Kokoro install manifest is invalid")]
    InvalidManifest,
    #[error("Kokoro install storage failed: {0}")]
    Platform(lettuce_platform::PlatformError),
    #[error("Kokoro install bytes failed verification")]
    Mismatch,
    #[error("Kokoro install artifact is unreadable")]
    Unreadable,
    #[error("Kokoro installed artifact is invalid")]
    InvalidArtifact,
}

#[cfg(test)]
mod tests {
    use lettuce_types::OperationId;

    use super::*;

    #[test]
    fn pinned_bundle_preserves_exact_four_file_plan() {
        let bundle = pinned_kokoro_model(KokoroModelVariant::Int8);
        assert_eq!(bundle.source_revision, KOKORO_SOURCE_REVISION);
        assert_eq!(bundle.artifacts.len(), 4);
        assert_eq!(bundle.artifacts[0].remote_path, "config.json");
        assert_eq!(bundle.artifacts[1].remote_path, "tokenizer.json");
        assert_eq!(bundle.artifacts[2].remote_path, "tokenizer_config.json");
        assert_eq!(bundle.artifacts[3].remote_path, "onnx/model_quantized.onnx");
        assert_eq!(bundle.artifacts[3].byte_size, 92_361_116);
        bundle.validate().expect("pinned bundle");
    }

    #[test]
    fn resumes_and_verifies_each_confined_bundle_artifact() {
        let root = std::env::temp_dir().join(format!("kokoro-install-{}", OperationId::new()));
        let store = KokoroInstallStore::open(&root).expect("install store");
        let remote = pinned_kokoro_model(KokoroModelVariant::Int8).artifacts[0].clone();
        let bytes = b"{\n  \"model_type\": \"style_text_to_speech_2\"\n}";
        let KokoroArtifactPreparation::Download(mut first) = store
            .prepare(KOKORO_SOURCE_REVISION, remote.clone())
            .expect("first preparation")
        else {
            panic!("expected download");
        };
        first.append(&bytes[..10]).expect("partial append");
        drop(first);
        let KokoroArtifactPreparation::Download(mut resumed) = store
            .prepare(KOKORO_SOURCE_REVISION, remote.clone())
            .expect("resume preparation")
        else {
            panic!("expected resumed download");
        };
        assert_eq!(resumed.offset(), 10);
        resumed.append(&bytes[10..]).expect("remaining append");
        let installed = resumed.finish().expect("verified install");
        assert_eq!(installed.byte_size, 44);
        assert!(installed.path.ends_with("config.json"));
        assert!(matches!(
            store
                .prepare(KOKORO_SOURCE_REVISION, remote)
                .expect("installed replay"),
            KokoroArtifactPreparation::Installed(_)
        ));
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
