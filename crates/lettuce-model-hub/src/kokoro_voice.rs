use std::{fmt, io::Read, path::Path, sync::Arc};

use lettuce_platform::{ConfinedInstallStore, InstallPreparation, ObjectKey, ResumableInstall};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    InstalledKokoroVoice, InstalledModelArtifact, KOKORO_SOURCE_REVISION, KokoroInstallError,
    ModelArtifactError,
};

pub const MAX_KOKORO_VOICE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_KOKORO_VOICE_MANIFEST_BYTES: u64 = 4 * 1024;
const MAX_KOKORO_VOICE_DIRECTORY_ENTRIES: usize = 1_024;
const KOKORO_VOICE_MANIFEST_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteKokoroVoice {
    pub id: String,
    pub remote_path: String,
    pub source_revision: String,
    pub byte_size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledKokoroVoiceManifest {
    version: u16,
    voice: RemoteKokoroVoice,
}

#[derive(Debug)]
pub struct KokoroVoiceInstallStore {
    inner: Arc<ConfinedInstallStore>,
}

pub struct MaterializedKokoroVoice {
    id: String,
    bytes: Box<[u8]>,
}

impl MaterializedKokoroVoice {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for MaterializedKokoroVoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaterializedKokoroVoice")
            .field("id", &self.id)
            .field("byte_len", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug)]
pub enum KokoroVoicePreparation {
    Installed(InstalledKokoroVoice),
    Download(KokoroVoiceDownloadSession),
}

#[derive(Debug)]
pub struct KokoroVoiceDownloadSession {
    inner: ResumableInstall,
    store: Arc<ConfinedInstallStore>,
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
            inner: Arc::new(
                ConfinedInstallStore::open(root).map_err(KokoroInstallError::Platform)?,
            ),
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
                write_manifest(&self.inner, &remote)?;
                Ok(KokoroVoicePreparation::Installed(InstalledKokoroVoice {
                    id: remote.id,
                    artifact,
                }))
            }
            InstallPreparation::Resume(inner) => Ok(KokoroVoicePreparation::Download(
                KokoroVoiceDownloadSession {
                    inner,
                    store: Arc::clone(&self.inner),
                    remote,
                },
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

    pub fn materialize(
        &self,
        remotes: &[RemoteKokoroVoice],
    ) -> Result<Option<Vec<MaterializedKokoroVoice>>, KokoroInstallError> {
        let mut materialized = Vec::with_capacity(remotes.len());
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
            let mut bytes = Vec::with_capacity(
                usize::try_from(remote.byte_size).map_err(|_| KokoroInstallError::Mismatch)?,
            );
            verify_voice_with(&mut file, remote, |chunk| bytes.extend_from_slice(chunk))?;
            materialized.push(MaterializedKokoroVoice {
                id: remote.id.clone(),
                bytes: bytes.into_boxed_slice(),
            });
        }
        Ok(Some(materialized))
    }

    pub fn installed_descriptors(&self) -> Result<Vec<RemoteKokoroVoice>, KokoroInstallError> {
        let directory =
            ObjectKey::from_segments(["voices"]).map_err(KokoroInstallError::Platform)?;
        let entries = self
            .inner
            .list(&directory, MAX_KOKORO_VOICE_DIRECTORY_ENTRIES)
            .map_err(KokoroInstallError::Platform)?;
        let mut descriptors = Vec::new();
        for entry in entries {
            let Some(id) = entry.name.strip_suffix(".manifest.json") else {
                continue;
            };
            if !entry.is_file || entry.len == 0 || entry.len > MAX_KOKORO_VOICE_MANIFEST_BYTES {
                continue;
            }
            let Some(remote) = read_manifest(&self.inner, id)? else {
                continue;
            };
            let target = voice_target(&remote.id)?;
            let Some(mut file) = self
                .inner
                .inspect(&target)
                .map_err(KokoroInstallError::Platform)?
            else {
                continue;
            };
            if verify_voice(&mut file, &remote).is_ok() {
                descriptors.push(remote);
            }
        }
        descriptors.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(descriptors)
    }

    pub fn remove_managed(&self, remote: &RemoteKokoroVoice) -> Result<bool, KokoroInstallError> {
        remote.validate()?;
        let target = voice_target(&remote.id)?;
        let installed = self
            .inner
            .inspect(&target)
            .map_err(KokoroInstallError::Platform)?;
        let manifest_target = manifest_target(&remote.id)?;
        let manifest = self
            .inner
            .inspect(&manifest_target)
            .map_err(KokoroInstallError::Platform)?;
        let had_installed = installed.is_some();
        let had_manifest = manifest.is_some();
        if had_manifest && read_manifest(&self.inner, &remote.id)?.as_ref() != Some(remote) {
            return Err(KokoroInstallError::Mismatch);
        }
        if let Some(mut file) = installed {
            verify_voice(&mut file, remote)?;
            let path = file.native_path().to_path_buf();
            drop(file);
            self.inner
                .remove_installed(&target, &path)
                .map_err(KokoroInstallError::Platform)?;
        }
        if let Some(file) = manifest {
            let path = file.native_path().to_path_buf();
            drop(file);
            self.inner
                .remove_installed(&manifest_target, &path)
                .map_err(KokoroInstallError::Platform)?;
        }
        Ok(had_installed || had_manifest)
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
        write_manifest(&self.store, &self.remote)?;
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

fn voice_target(id: &str) -> Result<ObjectKey, KokoroInstallError> {
    let filename = format!("{id}.bin");
    ObjectKey::from_segments(["voices", filename.as_str()]).map_err(KokoroInstallError::Platform)
}

fn manifest_target(id: &str) -> Result<ObjectKey, KokoroInstallError> {
    let filename = format!("{id}.manifest.json");
    ObjectKey::from_segments(["voices", filename.as_str()]).map_err(KokoroInstallError::Platform)
}

fn write_manifest(
    store: &ConfinedInstallStore,
    remote: &RemoteKokoroVoice,
) -> Result<(), KokoroInstallError> {
    let bytes = serde_json::to_vec(&InstalledKokoroVoiceManifest {
        version: KOKORO_VOICE_MANIFEST_VERSION,
        voice: remote.clone(),
    })
    .map_err(|_| KokoroInstallError::InvalidManifest)?;
    if bytes.is_empty()
        || u64::try_from(bytes.len()).map_err(|_| KokoroInstallError::InvalidManifest)?
            > MAX_KOKORO_VOICE_MANIFEST_BYTES
    {
        return Err(KokoroInstallError::InvalidManifest);
    }
    let partial_name = format!("{}.manifest.part", voice_identity(remote).to_hex());
    let partial = ObjectKey::from_segments(["downloads", partial_name.as_str()])
        .map_err(KokoroInstallError::Platform)?;
    let target = manifest_target(&remote.id)?;
    match store
        .prepare(partial, target, MAX_KOKORO_VOICE_MANIFEST_BYTES)
        .map_err(KokoroInstallError::Platform)?
    {
        InstallPreparation::Installed(mut file) => verify_manifest_bytes(&mut file, &bytes),
        InstallPreparation::Resume(mut install) => {
            install.restart().map_err(KokoroInstallError::Platform)?;
            install
                .append(&bytes)
                .map_err(KokoroInstallError::Platform)?;
            install.sync().map_err(KokoroInstallError::Platform)?;
            install.rewind().map_err(KokoroInstallError::Platform)?;
            verify_manifest_bytes(&mut install, &bytes)?;
            install.commit().map_err(KokoroInstallError::Platform)?;
            Ok(())
        }
    }
}

fn read_manifest(
    store: &ConfinedInstallStore,
    id: &str,
) -> Result<Option<RemoteKokoroVoice>, KokoroInstallError> {
    if !is_valid_kokoro_voice_id(id) {
        return Ok(None);
    }
    let target = manifest_target(id)?;
    let Some(mut file) = store
        .inspect(&target)
        .map_err(KokoroInstallError::Platform)?
    else {
        return Ok(None);
    };
    if file.is_empty() || file.len() > MAX_KOKORO_VOICE_MANIFEST_BYTES {
        return Ok(None);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(file.len()).map_err(|_| KokoroInstallError::InvalidManifest)?,
    );
    file.read_to_end(&mut bytes)
        .map_err(|_| KokoroInstallError::Unreadable)?;
    let Ok(manifest) = serde_json::from_slice::<InstalledKokoroVoiceManifest>(&bytes) else {
        return Ok(None);
    };
    if manifest.version != KOKORO_VOICE_MANIFEST_VERSION
        || manifest.voice.id != id
        || manifest.voice.validate().is_err()
    {
        return Ok(None);
    }
    Ok(Some(manifest.voice))
}

fn verify_manifest_bytes(file: &mut impl Read, expected: &[u8]) -> Result<(), KokoroInstallError> {
    let mut bytes = Vec::new();
    file.take(MAX_KOKORO_VOICE_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| KokoroInstallError::Unreadable)?;
    if bytes != expected {
        return Err(KokoroInstallError::Mismatch);
    }
    Ok(())
}

fn verify_voice(
    file: &mut impl Read,
    remote: &RemoteKokoroVoice,
) -> Result<(), KokoroInstallError> {
    verify_voice_with(file, remote, |_| {})
}

fn verify_voice_with(
    file: &mut impl Read,
    remote: &RemoteKokoroVoice,
    mut consume: impl FnMut(&[u8]),
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
        consume(&buffer[..read]);
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
        drop(store);
        let store = KokoroVoiceInstallStore::open(&root).expect("reopened install store");
        let descriptors = store.installed_descriptors().expect("offline catalog");
        assert_eq!(descriptors.len(), 1);
        assert_eq!(descriptors[0], remote);
        let materialized = store
            .materialize(std::slice::from_ref(&remote))
            .expect("materialization")
            .expect("installed voice");
        assert_eq!(materialized[0].id(), "af_heart");
        assert_eq!(materialized[0].bytes(), bytes);
        assert!(!format!("{:?}", materialized[0]).contains("verified voice bytes"));
        assert_eq!(
            store
                .installed(std::slice::from_ref(&remote))
                .expect("installed lookup")
                .expect("installed voice")[0]
                .artifact,
            installed.artifact
        );
        assert!(store.remove_managed(&remote).expect("remove voice"));
        assert!(!root.join("voices/af_heart.manifest.json").exists());
        assert!(!store.remove_managed(&remote).expect("removal replay"));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn offline_catalog_retains_unverified_voice_files() {
        let root = std::env::temp_dir().join(format!(
            "kokoro-voice-unverified-{}",
            lettuce_types::OperationId::new()
        ));
        let voices = root.join("voices");
        std::fs::create_dir_all(&voices).expect("voices directory");
        let orphan = voices.join("af_orphan.bin");
        std::fs::write(&orphan, b"retained voice bytes").expect("orphan voice");
        std::fs::write(
            voices.join("af_broken.manifest.json"),
            b"not a voice manifest",
        )
        .expect("broken manifest");
        let store = KokoroVoiceInstallStore::open(&root).expect("install store");

        assert!(
            store
                .installed_descriptors()
                .expect("offline catalog")
                .is_empty()
        );
        assert_eq!(
            std::fs::read(&orphan).expect("retained orphan"),
            b"retained voice bytes"
        );
        assert!(voices.join("af_broken.manifest.json").exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn offline_catalog_rejects_changed_bytes_without_deleting_them() {
        let root = std::env::temp_dir().join(format!(
            "kokoro-voice-changed-{}",
            lettuce_types::OperationId::new()
        ));
        let original = b"verified voice bytes";
        let remote = RemoteKokoroVoice::pinned(
            "af_heart",
            KOKORO_SOURCE_REVISION,
            u64::try_from(original.len()).expect("voice size"),
            format!("{:x}", Sha256::digest(original)),
        )
        .expect("remote voice");
        let store = KokoroVoiceInstallStore::open(&root).expect("install store");
        let KokoroVoicePreparation::Download(mut download) =
            store.prepare(remote).expect("preparation")
        else {
            panic!("expected download");
        };
        download.append(original).expect("voice bytes");
        download.finish().expect("verified voice");
        let changed = b"modified voice bytes";
        assert_eq!(changed.len(), original.len());
        let path = root.join("voices/af_heart.bin");
        std::fs::write(&path, changed).expect("changed voice");

        assert!(
            store
                .installed_descriptors()
                .expect("offline catalog")
                .is_empty()
        );
        assert_eq!(std::fs::read(&path).expect("retained voice"), changed);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
