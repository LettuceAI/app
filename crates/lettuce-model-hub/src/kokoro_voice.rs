use std::{io::Read, path::Path};

use lettuce_platform::{ConfinedInstallStore, InstallPreparation, ObjectKey, ResumableInstall};
use sha2::{Digest, Sha256};

use crate::{
    InstalledKokoroVoice, InstalledModelArtifact, KOKORO_SOURCE_REVISION, KokoroInstallError,
    ModelArtifactError,
};

pub const MAX_KOKORO_VOICE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteKokoroVoice {
    pub id: String,
    pub remote_path: String,
    pub source_revision: String,
    pub byte_size: u64,
    pub sha256: String,
}

#[derive(Debug)]
pub struct KokoroVoiceInstallStore {
    inner: ConfinedInstallStore,
}

#[derive(Debug)]
pub enum KokoroVoicePreparation {
    Installed(InstalledKokoroVoice),
    Download(KokoroVoiceDownloadSession),
}

#[derive(Debug)]
pub struct KokoroVoiceDownloadSession {
    inner: ResumableInstall,
    remote: RemoteKokoroVoice,
}

impl RemoteKokoroVoice {
    pub fn pinned(
        id: impl Into<String>,
        source_revision: impl Into<String>,
        byte_size: u64,
        sha256: impl Into<String>,
    ) -> Result<Self, KokoroInstallError> {
        let id = id.into();
        let voice = Self {
            remote_path: format!("voices/{id}.bin"),
            id,
            source_revision: source_revision.into(),
            byte_size,
            sha256: sha256.into(),
        };
        voice.validate()?;
        Ok(voice)
    }

    pub fn validate(&self) -> Result<(), KokoroInstallError> {
        if !is_valid_kokoro_voice_id(&self.id)
            || self.remote_path != format!("voices/{}.bin", self.id)
            || self.source_revision != KOKORO_SOURCE_REVISION
            || self.byte_size == 0
            || self.byte_size > MAX_KOKORO_VOICE_BYTES
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || self.sha256 != self.sha256.to_ascii_lowercase()
        {
            return Err(KokoroInstallError::InvalidManifest);
        }
        Ok(())
    }
}

impl KokoroVoiceInstallStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, KokoroInstallError> {
        Ok(Self {
            inner: ConfinedInstallStore::open(root).map_err(KokoroInstallError::Platform)?,
        })
    }

    pub fn prepare(
        &self,
        remote: RemoteKokoroVoice,
    ) -> Result<KokoroVoicePreparation, KokoroInstallError> {
        remote.validate()?;
        let partial = ObjectKey::from_segments([
            "downloads",
            format!("{}.part", voice_identity(&remote).to_hex()).as_str(),
        ])
        .map_err(KokoroInstallError::Platform)?;
        let filename = format!("{}.bin", remote.id);
        let target = ObjectKey::from_segments(["voices", filename.as_str()])
            .map_err(KokoroInstallError::Platform)?;
        match self
            .inner
            .prepare(partial, target, remote.byte_size)
            .map_err(KokoroInstallError::Platform)?
        {
            InstallPreparation::Installed(mut file) => {
                verify_voice(&mut file, &remote)?;
                let artifact = InstalledModelArtifact::inspect(file.native_path().to_path_buf())
                    .map_err(map_artifact_error)?;
                Ok(KokoroVoicePreparation::Installed(InstalledKokoroVoice {
                    id: remote.id,
                    artifact,
                }))
            }
            InstallPreparation::Resume(inner) => Ok(KokoroVoicePreparation::Download(
                KokoroVoiceDownloadSession { inner, remote },
            )),
        }
    }

    pub fn installed(
        &self,
        remotes: &[RemoteKokoroVoice],
    ) -> Result<Option<Vec<InstalledKokoroVoice>>, KokoroInstallError> {
        let mut installed = Vec::with_capacity(remotes.len());
        for remote in remotes {
            remote.validate()?;
            let filename = format!("{}.bin", remote.id);
            let target = ObjectKey::from_segments(["voices", filename.as_str()])
                .map_err(KokoroInstallError::Platform)?;
            let Some(mut file) = self
                .inner
                .inspect(&target)
                .map_err(KokoroInstallError::Platform)?
            else {
                return Ok(None);
            };
            verify_voice(&mut file, remote)?;
            let artifact = InstalledModelArtifact::inspect(file.native_path().to_path_buf())
                .map_err(map_artifact_error)?;
            installed.push(InstalledKokoroVoice {
                id: remote.id.clone(),
                artifact,
            });
        }
        Ok(Some(installed))
    }

    pub fn remove_managed(&self, remote: &RemoteKokoroVoice) -> Result<bool, KokoroInstallError> {
        remote.validate()?;
        let filename = format!("{}.bin", remote.id);
        let target = ObjectKey::from_segments(["voices", filename.as_str()])
            .map_err(KokoroInstallError::Platform)?;
        let Some(mut file) = self
            .inner
            .inspect(&target)
            .map_err(KokoroInstallError::Platform)?
        else {
            return Ok(false);
        };
        verify_voice(&mut file, remote)?;
        let path = file.native_path().to_path_buf();
        drop(file);
        self.inner
            .remove_installed(&target, &path)
            .map_err(KokoroInstallError::Platform)
    }
}

impl KokoroVoiceDownloadSession {
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

    pub fn finish(mut self) -> Result<InstalledKokoroVoice, KokoroInstallError> {
        if self.inner.offset() != self.remote.byte_size {
            return Err(KokoroInstallError::Mismatch);
        }
        self.inner.sync().map_err(KokoroInstallError::Platform)?;
        self.inner.rewind().map_err(KokoroInstallError::Platform)?;
        verify_voice(&mut self.inner, &self.remote)?;
        let path = self.inner.commit().map_err(KokoroInstallError::Platform)?;
        let artifact = InstalledModelArtifact::inspect(path).map_err(map_artifact_error)?;
        Ok(InstalledKokoroVoice {
            id: self.remote.id,
            artifact,
        })
    }
}

#[must_use]
pub fn is_valid_kokoro_voice_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn voice_identity(remote: &RemoteKokoroVoice) -> blake3::Hash {
    let mut hash = blake3::Hasher::new();
    hash.update(remote.source_revision.as_bytes());
    hash.update(&[0]);
    hash.update(remote.remote_path.as_bytes());
    hash.update(&[0]);
    hash.update(remote.sha256.as_bytes());
    hash.update(&remote.byte_size.to_le_bytes());
    hash.finalize()
}

fn verify_voice(
    file: &mut impl Read,
    remote: &RemoteKokoroVoice,
) -> Result<(), KokoroInstallError> {
    let mut hash = Sha256::new();
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
        hash.update(&buffer[..read]);
    }
    if total != remote.byte_size || format!("{:x}", hash.finalize()) != remote.sha256 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_voice_derives_the_only_managed_remote_path() {
        let voice = RemoteKokoroVoice::pinned(
            "af_heart",
            KOKORO_SOURCE_REVISION,
            522_240,
            "d583ccff3cdca2f7fae535cb998ac07e9fcb90f09737b9a41fa2734ec44a8f0b",
        )
        .expect("remote voice");
        assert_eq!(voice.remote_path, "voices/af_heart.bin");
        assert!(voice.validate().is_ok());
        assert!(
            RemoteKokoroVoice::pinned(
                "../af_heart",
                KOKORO_SOURCE_REVISION,
                522_240,
                "d5".repeat(32),
            )
            .is_err()
        );
        assert!(RemoteKokoroVoice::pinned("af_heart", "main", 522_240, "d5".repeat(32),).is_err());
    }

    #[test]
    fn voice_install_resumes_verifies_and_replays() {
        let root = std::env::temp_dir().join(format!(
            "kokoro-voice-install-{}",
            lettuce_types::OperationId::new()
        ));
        let bytes = b"verified voice bytes";
        let remote = RemoteKokoroVoice::pinned(
            "af_heart",
            KOKORO_SOURCE_REVISION,
            u64::try_from(bytes.len()).expect("voice size"),
            format!("{:x}", Sha256::digest(bytes)),
        )
        .expect("remote voice");
        let store = KokoroVoiceInstallStore::open(&root).expect("install store");
        let KokoroVoicePreparation::Download(mut first) =
            store.prepare(remote.clone()).expect("first preparation")
        else {
            panic!("expected download");
        };
        first.append(&bytes[..8]).expect("partial voice");
        drop(first);
        let KokoroVoicePreparation::Download(mut resumed) =
            store.prepare(remote.clone()).expect("resumed preparation")
        else {
            panic!("expected resumed download");
        };
        assert_eq!(resumed.offset(), 8);
        resumed.append(&bytes[8..]).expect("remaining voice");
        let installed = resumed.finish().expect("verified voice");
        assert_eq!(installed.id, "af_heart");
        assert_eq!(
            store
                .installed(std::slice::from_ref(&remote))
                .expect("installed lookup")
                .expect("installed voice")[0]
                .artifact,
            installed.artifact
        );
        assert!(store.remove_managed(&remote).expect("remove voice"));
        assert!(!store.remove_managed(&remote).expect("removal replay"));
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
