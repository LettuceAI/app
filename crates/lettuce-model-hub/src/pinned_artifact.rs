//! Downloads of artifacts whose size (and usually SHA-256) is known up
//! front, confined below one install root. Partial bytes resume by the
//! artifact's complete identity and are verified before they become visible.

use std::{
    io::Read,
    path::{Path, PathBuf},
};

use lettuce_platform::{ConfinedInstallStore, InstallPreparation, ObjectKey, ResumableInstall};
use sha2::{Digest, Sha256};

/// One file to install: where it comes from (the identity that names its
/// partial download), where it lands and how it is verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedArtifact {
    pub source_identity: String,
    pub local_segments: Vec<String>,
    pub byte_size: u64,
    /// Lowercase hex SHA-256; `None` checks the size only, for release assets
    /// published without a digest.
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PinnedArtifactError {
    #[error("artifact description is invalid")]
    InvalidArtifact,
    #[error("artifact storage failed: {0}")]
    Platform(lettuce_platform::PlatformError),
    #[error("artifact bytes failed verification")]
    Mismatch,
    #[error("artifact is unreadable")]
    Unreadable,
    #[error("this file is already being downloaded")]
    Busy,
}

static DOWNLOADS_IN_PROGRESS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashSet<String>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

/// Holds a partial file for one download at a time in this process.
#[derive(Debug)]
struct DownloadClaim(String);

impl DownloadClaim {
    fn take(key: String) -> Result<Self, PinnedArtifactError> {
        let mut claimed = DOWNLOADS_IN_PROGRESS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if claimed.insert(key.clone()) {
            Ok(Self(key))
        } else {
            Err(PinnedArtifactError::Busy)
        }
    }
}

impl Drop for DownloadClaim {
    fn drop(&mut self) {
        DOWNLOADS_IN_PROGRESS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.0);
    }
}

#[derive(Debug)]
pub enum PinnedArtifactPreparation {
    Installed(PathBuf),
    Download(PinnedDownload),
}

#[derive(Debug)]
pub struct PinnedArtifactStore {
    inner: ConfinedInstallStore,
    root: PathBuf,
}

#[derive(Debug)]
pub struct PinnedDownload {
    inner: ResumableInstall,
    artifact: PinnedArtifact,
    _claim: DownloadClaim,
}

impl PinnedArtifact {
    fn validate(&self) -> Result<ObjectKey, PinnedArtifactError> {
        if self.byte_size == 0
            || self.source_identity.is_empty()
            || self
                .local_segments
                .first()
                .is_some_and(|segment| segment == ".downloads")
            || self.sha256.as_deref().is_some_and(|sha256| {
                sha256.len() != 64
                    || !sha256
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
        {
            return Err(PinnedArtifactError::InvalidArtifact);
        }
        ObjectKey::from_segments(&self.local_segments)
            .map_err(|_| PinnedArtifactError::InvalidArtifact)
    }

    fn partial_key(&self) -> Result<ObjectKey, PinnedArtifactError> {
        let mut hash = blake3::Hasher::new();
        hash.update(self.source_identity.as_bytes());
        hash.update(&[0]);
        for segment in &self.local_segments {
            hash.update(segment.as_bytes());
            hash.update(&[0]);
        }
        hash.update(self.sha256.as_deref().unwrap_or_default().as_bytes());
        hash.update(&self.byte_size.to_le_bytes());
        ObjectKey::from_segments([
            ".downloads".to_owned(),
            format!("{}{}.part", self.partial_prefix(), hash.finalize().to_hex()),
        ])
        .map_err(PinnedArtifactError::Platform)
    }

    /// Every partial of one installed file starts with this, whatever
    /// revision it came from.
    fn partial_prefix(&self) -> String {
        let mut hash = blake3::Hasher::new();
        for segment in &self.local_segments {
            hash.update(segment.as_bytes());
            hash.update(&[0]);
        }
        format!("{}-", hash.finalize().to_hex())
    }
}

impl PinnedArtifactStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, PinnedArtifactError> {
        Ok(Self {
            inner: ConfinedInstallStore::open(root.as_ref())
                .map_err(PinnedArtifactError::Platform)?,
            root: root.as_ref().to_path_buf(),
        })
    }

    /// The installed file after a full verification, or a resumable
    /// download. An installed file that fails verification is replaced.
    pub fn prepare(
        &self,
        artifact: PinnedArtifact,
    ) -> Result<PinnedArtifactPreparation, PinnedArtifactError> {
        let target = artifact.validate()?;
        let partial = artifact.partial_key()?;
        let claim = DownloadClaim::take(format!("{}\0{target}", self.root.display()))?;
        if let Err(error) = self
            .inner
            .discard_partials_like(&partial, &artifact.partial_prefix())
        {
            tracing::warn!(%error, "failed to remove stale partial downloads");
        }
        match self
            .inner
            .prepare(partial.clone(), target.clone(), artifact.byte_size)
        {
            Ok(InstallPreparation::Installed(mut file)) => {
                if verify(&mut file, &artifact).is_ok() {
                    return Ok(PinnedArtifactPreparation::Installed(
                        file.native_path().to_path_buf(),
                    ));
                }
                let path = file.native_path().to_path_buf();
                drop(file);
                self.inner
                    .remove_installed(&target, &path)
                    .map_err(PinnedArtifactError::Platform)?;
            }
            Ok(InstallPreparation::Resume(inner)) => {
                return Ok(PinnedArtifactPreparation::Download(PinnedDownload {
                    inner,
                    artifact,
                    _claim: claim,
                }));
            }
            Err(lettuce_platform::PlatformError::LimitExceeded) => {
                self.inner
                    .discard(&target)
                    .map_err(PinnedArtifactError::Platform)?;
                self.inner
                    .discard(&partial)
                    .map_err(PinnedArtifactError::Platform)?;
            }
            Err(error) => return Err(PinnedArtifactError::Platform(error)),
        }
        match self
            .inner
            .prepare(partial, target, artifact.byte_size)
            .map_err(PinnedArtifactError::Platform)?
        {
            InstallPreparation::Resume(inner) => {
                Ok(PinnedArtifactPreparation::Download(PinnedDownload {
                    inner,
                    artifact,
                    _claim: claim,
                }))
            }
            InstallPreparation::Installed(_) => Err(PinnedArtifactError::Mismatch),
        }
    }

    /// The installed file when one of the expected size exists; the content
    /// is not rehashed.
    pub fn installed_path(
        &self,
        artifact: &PinnedArtifact,
    ) -> Result<Option<PathBuf>, PinnedArtifactError> {
        let target = artifact.validate()?;
        Ok(self
            .inner
            .inspect(&target)
            .map_err(PinnedArtifactError::Platform)?
            .filter(|file| file.len() == artifact.byte_size)
            .map(|file| file.native_path().to_path_buf()))
    }

    /// Removes an installed file; `false` when it was already gone.
    pub fn remove(&self, local_segments: &[String]) -> Result<bool, PinnedArtifactError> {
        let target = ObjectKey::from_segments(local_segments)
            .map_err(|_| PinnedArtifactError::InvalidArtifact)?;
        let Some(file) = self
            .inner
            .inspect(&target)
            .map_err(PinnedArtifactError::Platform)?
        else {
            return Ok(false);
        };
        let path = file.native_path().to_path_buf();
        drop(file);
        self.inner
            .remove_installed(&target, &path)
            .map_err(PinnedArtifactError::Platform)
    }
}

impl PinnedDownload {
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.inner.offset()
    }

    #[must_use]
    pub const fn artifact(&self) -> &PinnedArtifact {
        &self.artifact
    }

    pub fn restart(&mut self) -> Result<(), PinnedArtifactError> {
        self.inner.restart().map_err(PinnedArtifactError::Platform)
    }

    /// Deletes the partial download.
    pub fn discard(self) -> Result<(), PinnedArtifactError> {
        self.inner.discard().map_err(PinnedArtifactError::Platform)
    }

    pub fn append(&mut self, bytes: &[u8]) -> Result<u64, PinnedArtifactError> {
        self.inner
            .append(bytes)
            .map_err(PinnedArtifactError::Platform)
    }

    /// Verifies the complete partial file and moves it into place. A failed
    /// verification discards the partial bytes so a retry starts over.
    pub fn finish(mut self) -> Result<PathBuf, PinnedArtifactError> {
        if self.inner.offset() != self.artifact.byte_size {
            return Err(PinnedArtifactError::Mismatch);
        }
        self.inner.sync().map_err(PinnedArtifactError::Platform)?;
        self.inner.rewind().map_err(PinnedArtifactError::Platform)?;
        if let Err(error) = verify(&mut self.inner, &self.artifact) {
            self.inner
                .discard()
                .map_err(PinnedArtifactError::Platform)?;
            return Err(error);
        }
        self.inner.commit().map_err(PinnedArtifactError::Platform)
    }
}

/// Checks a downloaded file against the git blob id its repository lists,
/// for files published without an LFS SHA-256.
pub fn verify_git_blob(path: &Path, expected: &str) -> Result<(), PinnedArtifactError> {
    let mut file = std::fs::File::open(path).map_err(|_| PinnedArtifactError::Unreadable)?;
    let length = file
        .metadata()
        .map_err(|_| PinnedArtifactError::Unreadable)?
        .len();
    let mut hasher = sha1_smol::Sha1::new();
    hasher.update(format!("blob {length}\0").as_bytes());
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| PinnedArtifactError::Unreadable)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| PinnedArtifactError::Mismatch)?)
            .ok_or(PinnedArtifactError::Mismatch)?;
        hasher.update(&buffer[..read]);
    }
    if total != length || !hasher.digest().to_string().eq_ignore_ascii_case(expected) {
        return Err(PinnedArtifactError::Mismatch);
    }
    Ok(())
}

fn verify(file: &mut impl Read, artifact: &PinnedArtifact) -> Result<(), PinnedArtifactError> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| PinnedArtifactError::Unreadable)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| PinnedArtifactError::Mismatch)?)
            .ok_or(PinnedArtifactError::Mismatch)?;
        if total > artifact.byte_size {
            return Err(PinnedArtifactError::Mismatch);
        }
        if artifact.sha256.is_some() {
            hasher.update(&buffer[..read]);
        }
    }
    if total != artifact.byte_size
        || artifact
            .sha256
            .as_deref()
            .is_some_and(|sha256| format!("{:x}", hasher.finalize()) != sha256)
    {
        return Err(PinnedArtifactError::Mismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use lettuce_types::OperationId;

    use super::*;

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn artifact(bytes: &[u8]) -> PinnedArtifact {
        PinnedArtifact {
            source_identity: "hf:owner/repo@rev/split_files/vae/ae.safetensors".to_owned(),
            local_segments: vec![
                "components".to_owned(),
                sha256(bytes),
                "ae.safetensors".to_owned(),
            ],
            byte_size: bytes.len() as u64,
            sha256: Some(sha256(bytes)),
        }
    }

    #[test]
    fn git_blob_ids_verify_files_published_without_lfs() {
        let path = std::env::temp_dir().join(format!("pinned-blob-{}", OperationId::new()));
        std::fs::write(&path, b"hello\n").expect("file");
        assert_eq!(
            verify_git_blob(&path, "CE013625030BA8DBA906F756967F9E9CA394464A"),
            Ok(())
        );
        assert_eq!(
            verify_git_blob(&path, "0000000000000000000000000000000000000000"),
            Err(PinnedArtifactError::Mismatch)
        );
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn a_file_is_downloaded_by_one_install_at_a_time() {
        let root = std::env::temp_dir().join(format!("pinned-busy-{}", OperationId::new()));
        let store = PinnedArtifactStore::open(&root).expect("store");
        let expected = artifact(b"shared bytes");
        let first = store.prepare(expected.clone()).expect("first");
        assert!(matches!(
            store.prepare(expected.clone()),
            Err(PinnedArtifactError::Busy)
        ));
        drop(first);
        assert!(store.prepare(expected).is_ok());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn downloads_resume_verify_and_replace_corrupt_installs() {
        let root = std::env::temp_dir().join(format!("pinned-artifact-{}", OperationId::new()));
        let store = PinnedArtifactStore::open(&root).expect("store");
        let bytes = b"verified component bytes";
        let expected = artifact(bytes);
        let PinnedArtifactPreparation::Download(mut download) =
            store.prepare(expected.clone()).expect("prepare")
        else {
            panic!("expected a download");
        };
        download.append(&bytes[..8]).expect("append");
        drop(download);
        let PinnedArtifactPreparation::Download(mut download) =
            store.prepare(expected.clone()).expect("resume")
        else {
            panic!("expected a resumed download");
        };
        assert_eq!(download.offset(), 8);
        download.append(&bytes[8..]).expect("append rest");
        let path = download.finish().expect("finish");
        assert_eq!(std::fs::read(&path).expect("installed"), bytes);
        assert_eq!(store.installed_path(&expected), Ok(Some(path.clone())));
        assert!(matches!(
            store.prepare(expected.clone()),
            Ok(PinnedArtifactPreparation::Installed(installed)) if installed == path
        ));

        std::fs::write(&path, b"an oversized installed file of the wrong size").expect("oversize");
        assert!(matches!(
            store.prepare(expected.clone()),
            Ok(PinnedArtifactPreparation::Download(download)) if download.offset() == 0
        ));
        let PinnedArtifactPreparation::Download(mut download) =
            store.prepare(expected.clone()).expect("prepare")
        else {
            panic!("expected a download");
        };
        download.append(bytes).expect("append");
        let path = download.finish().expect("finish");
        std::fs::write(&path, b"corrupted component byte").expect("corrupt");
        assert!(matches!(
            store.prepare(expected.clone()),
            Ok(PinnedArtifactPreparation::Download(download)) if download.offset() == 0
        ));
        assert_eq!(store.installed_path(&expected), Ok(None));

        let PinnedArtifactPreparation::Download(mut wrong) =
            store.prepare(expected.clone()).expect("prepare again")
        else {
            panic!("expected a download");
        };
        wrong.append(b"wrong bytes, equal size!").expect("append");
        assert_eq!(wrong.finish(), Err(PinnedArtifactError::Mismatch));
        assert!(matches!(
            store.prepare(expected),
            Ok(PinnedArtifactPreparation::Download(download)) if download.offset() == 0
        ));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn stale_revision_and_abandoned_partials_are_removed() {
        let root = std::env::temp_dir().join(format!("pinned-artifact-{}", OperationId::new()));
        let store = PinnedArtifactStore::open(&root).expect("store");
        let bytes = b"verified component bytes";
        let old = artifact(bytes);
        let PinnedArtifactPreparation::Download(mut download) =
            store.prepare(old.clone()).expect("prepare")
        else {
            panic!("expected a download");
        };
        download.append(&bytes[..8]).expect("append");
        drop(download);
        let partials = || {
            std::fs::read_dir(root.join(".downloads"))
                .expect("downloads")
                .count()
        };
        assert_eq!(partials(), 1);
        let moved = PinnedArtifact {
            source_identity: "hf:owner/repo@newrev/split_files/vae/ae.safetensors".to_owned(),
            ..old
        };
        let PinnedArtifactPreparation::Download(mut download) =
            store.prepare(moved).expect("prepare moved revision")
        else {
            panic!("expected a download");
        };
        assert_eq!(download.offset(), 0);
        download.append(&bytes[..4]).expect("append");
        assert_eq!(partials(), 1);
        download.discard().expect("discard");
        assert_eq!(partials(), 0);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn digestless_artifacts_check_the_size_and_bad_descriptions_are_refused() {
        let root = std::env::temp_dir().join(format!("pinned-artifact-{}", OperationId::new()));
        let store = PinnedArtifactStore::open(&root).expect("store");
        let unhashed = PinnedArtifact {
            source_identity: "https://github.com/leejet/stable-diffusion.cpp/releases/x.zip"
                .to_owned(),
            local_segments: vec!["downloads".to_owned(), "x.zip".to_owned()],
            byte_size: 4,
            sha256: None,
        };
        let PinnedArtifactPreparation::Download(mut download) =
            store.prepare(unhashed.clone()).expect("prepare")
        else {
            panic!("expected a download");
        };
        download.append(b"zip!").expect("append");
        let path = download.finish().expect("finish");
        assert!(path.ends_with("downloads/x.zip"));
        assert_eq!(store.remove(&unhashed.local_segments), Ok(true));
        assert_eq!(store.remove(&unhashed.local_segments), Ok(false));
        for invalid in [
            PinnedArtifact {
                byte_size: 0,
                ..unhashed.clone()
            },
            PinnedArtifact {
                sha256: Some("ABC".to_owned()),
                ..unhashed.clone()
            },
            PinnedArtifact {
                local_segments: vec!["..".to_owned()],
                ..unhashed.clone()
            },
        ] {
            assert_eq!(
                store.prepare(invalid).err(),
                Some(PinnedArtifactError::InvalidArtifact)
            );
        }
        std::fs::remove_dir_all(root).ok();
    }
}
