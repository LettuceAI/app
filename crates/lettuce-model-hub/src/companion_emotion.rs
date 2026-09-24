use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use lettuce_platform::{ConfinedInstallStore, InstallPreparation, ObjectKey, PlatformError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    HfPinnedFiles, InstalledModelArtifact, ModelArtifactError, PinnedArtifact, PinnedArtifactError,
    PinnedArtifactStore, verify_git_blob,
};

use super::embedding::verify_artifact;

pub const THYMOS_REPOSITORY: &str = "Zeolit/lettuce-thymos-26m-v1";
pub const THYMOS_MODEL_PATH: &str = "onnx/model_quantized.onnx";
pub const THYMOS_TOKENIZER_PATH: &str = "tokenizer.json";
pub const THYMOS_LABELS_PATH: &str = "labels.json";
pub const THYMOS_FILES: [&str; 3] = [THYMOS_MODEL_PATH, THYMOS_TOKENIZER_PATH, THYMOS_LABELS_PATH];

const MAX_LABELS_FILE_BYTES: u64 = 1024 * 1024;
const MAX_INSTALLED_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_WINDOW_TOKENS: usize = 4096;
const INSTALLED_MANIFEST_FILE: &str = "installed.json";
const ACTIVE_INSTALL_FILE: &str = "active-install.json";
const STAGED_SUFFIX: &str = ".next";
const DOWNLOADS_DIRECTORY: &str = ".downloads";
const MAX_DIRECTORY_ENTRIES: usize = 1024;
const REVISIONS_DIRECTORY: &str = "revisions";

/// One Thymos file at a pinned revision, as the Hugging Face API reports it.
/// LFS files carry their SHA-256; files stored in git (`labels.json`) carry
/// their git blob id instead, checked once the download is complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCompanionEmotionArtifact {
    pub remote_path: &'static str,
    pub byte_size: u64,
    pub sha256: Option<String>,
    pub git_blob_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCompanionEmotionModel {
    pub source_revision: String,
    pub model: RemoteCompanionEmotionArtifact,
    pub tokenizer: RemoteCompanionEmotionArtifact,
    pub labels: RemoteCompanionEmotionArtifact,
}

impl RemoteCompanionEmotionModel {
    /// The three Thymos files as the shared Hugging Face pin
    /// ([`crate::pinned_files`] over [`THYMOS_FILES`]) reports them: its
    /// commit becomes the revision every file is fetched at.
    pub fn from_pinned_files(pinned: &HfPinnedFiles) -> Result<Self, CompanionEmotionInstallError> {
        let artifact = |remote_path: &'static str| {
            pinned
                .files
                .iter()
                .find(|file| file.path == remote_path)
                .map(|file| RemoteCompanionEmotionArtifact {
                    remote_path,
                    byte_size: file.size,
                    sha256: file.sha256.clone(),
                    git_blob_id: file.git_blob_id.clone(),
                })
                .ok_or(CompanionEmotionInstallError::InvalidRemote)
        };
        let model = Self {
            source_revision: pinned.revision.clone(),
            model: artifact(THYMOS_MODEL_PATH)?,
            tokenizer: artifact(THYMOS_TOKENIZER_PATH)?,
            labels: artifact(THYMOS_LABELS_PATH)?,
        };
        model.validate()?;
        Ok(model)
    }

    /// The model and tokenizer are LFS files and must carry their SHA-256;
    /// every file carries a SHA-256 or a git blob id; the revision must be an
    /// immutable forty-character commit.
    pub fn validate(&self) -> Result<(), CompanionEmotionInstallError> {
        if !is_commit(&self.source_revision)
            || self.model.remote_path != THYMOS_MODEL_PATH
            || self.tokenizer.remote_path != THYMOS_TOKENIZER_PATH
            || self.labels.remote_path != THYMOS_LABELS_PATH
            || self.model.sha256.is_none()
            || self.tokenizer.sha256.is_none()
            || self.labels.byte_size > MAX_LABELS_FILE_BYTES
        {
            return Err(CompanionEmotionInstallError::InvalidRemote);
        }
        for artifact in self.artifacts() {
            if artifact.byte_size == 0
                || artifact
                    .sha256
                    .as_deref()
                    .is_some_and(|sha| !is_sha256(sha))
                || artifact
                    .git_blob_id
                    .as_deref()
                    .is_some_and(|id| !is_commit(id))
                || (artifact.sha256.is_none() && artifact.git_blob_id.is_none())
            {
                return Err(CompanionEmotionInstallError::InvalidRemote);
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn artifacts(&self) -> [&RemoteCompanionEmotionArtifact; 3] {
        [&self.model, &self.tokenizer, &self.labels]
    }

    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.artifacts()
            .iter()
            .map(|artifact| artifact.byte_size)
            .sum()
    }

    /// Where a file of this revision lands below the install root:
    /// `revisions/<revision>/<remote path>`.
    #[must_use]
    pub fn local_segments(&self, artifact: &RemoteCompanionEmotionArtifact) -> Vec<String> {
        local_segments(&self.source_revision, artifact.remote_path)
    }

    #[must_use]
    pub fn source_identity(&self, artifact: &RemoteCompanionEmotionArtifact) -> String {
        format!(
            "hf:{THYMOS_REPOSITORY}@{}/{}",
            self.source_revision, artifact.remote_path
        )
    }

    #[must_use]
    pub fn pinned_artifact(&self, artifact: &RemoteCompanionEmotionArtifact) -> PinnedArtifact {
        PinnedArtifact {
            source_identity: self.source_identity(artifact),
            local_segments: self.local_segments(artifact),
            byte_size: artifact.byte_size,
            sha256: artifact.sha256.clone(),
        }
    }
}

fn local_segments(revision: &str, remote_path: &str) -> Vec<String> {
    [REVISIONS_DIRECTORY.to_owned(), revision.to_owned()]
        .into_iter()
        .chain(remote_path.split('/').map(str::to_owned))
        .collect()
}

fn is_commit(revision: &str) -> bool {
    revision.len() == 40
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledCompanionEmotionManifest {
    /// Immutable upstream commit the files were downloaded at.
    pub source_revision: String,
    pub model: InstalledModelArtifact,
    pub tokenizer: InstalledModelArtifact,
    pub labels: InstalledModelArtifact,
}

impl InstalledCompanionEmotionManifest {
    /// Rechecks every file's size and BLAKE3, reads the label metadata and,
    /// when `labels.json` names a `model_sha256`, checks the model against it
    /// as the upstream `inference.py` does.
    pub fn verify(&self) -> Result<VerifiedCompanionEmotionArtifacts, ModelArtifactError> {
        if !is_commit(&self.source_revision) {
            return Err(ModelArtifactError::InvalidManifest);
        }
        verify_artifact(&self.model)?;
        verify_artifact(&self.tokenizer)?;
        verify_artifact(&self.labels)?;
        let labels = CompanionEmotionLabels::read(&self.labels.path)?;
        if let Some(expected) = labels.model_sha256.as_deref()
            && sha256_file(&self.model.path)? != expected
        {
            return Err(ModelArtifactError::Mismatch);
        }
        Ok(VerifiedCompanionEmotionArtifacts {
            source_revision: self.source_revision.clone(),
            model_path: self.model.path.clone(),
            tokenizer_path: self.tokenizer.path.clone(),
            labels,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedCompanionEmotionArtifacts {
    pub source_revision: String,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub labels: CompanionEmotionLabels,
}

/// The label order, per-class decision thresholds and window length of a
/// Thymos `labels.json`, validated as the upstream `inference.py` does.
#[derive(Debug, Clone, PartialEq)]
pub struct CompanionEmotionLabels {
    labels: Vec<String>,
    thresholds: Vec<f32>,
    window: usize,
    model_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LabelsFile {
    labels: Vec<String>,
    thresholds: Vec<f64>,
    max_length: Option<usize>,
    #[serde(default)]
    model_sha256: Option<String>,
}

impl CompanionEmotionLabels {
    pub fn read(path: &Path) -> Result<Self, ModelArtifactError> {
        let file = File::open(path).map_err(|_| ModelArtifactError::Unreadable)?;
        let mut bytes = Vec::new();
        file.take(MAX_LABELS_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ModelArtifactError::Unreadable)?;
        if u64::try_from(bytes.len()).map_err(|_| ModelArtifactError::InvalidManifest)?
            > MAX_LABELS_FILE_BYTES
        {
            return Err(ModelArtifactError::InvalidManifest);
        }
        Self::parse(&bytes)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, ModelArtifactError> {
        let file: LabelsFile =
            serde_json::from_slice(bytes).map_err(|_| ModelArtifactError::InvalidManifest)?;
        let window = file.max_length.ok_or(ModelArtifactError::InvalidManifest)?;
        if file.labels.is_empty()
            || file.labels.len() != file.thresholds.len()
            || file.labels.iter().any(|label| label.trim().is_empty())
            || window > MAX_WINDOW_TOKENS
        {
            return Err(ModelArtifactError::InvalidManifest);
        }
        let thresholds = file
            .thresholds
            .iter()
            .map(|&value| {
                let value = value as f32;
                (value.is_finite() && (0.0..=1.0).contains(&value))
                    .then_some(value)
                    .ok_or(ModelArtifactError::InvalidManifest)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let model_sha256 = match file.model_sha256 {
            Some(value) if value.is_empty() => None,
            Some(value)
                if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
            {
                Some(value)
            }
            Some(_) => return Err(ModelArtifactError::InvalidManifest),
            None => None,
        };
        let labels = Self {
            labels: file.labels,
            thresholds,
            window,
            model_sha256,
        };
        let stride = labels.stride();
        if labels.window < 4 || stride == 0 || stride >= labels.window - 2 {
            return Err(ModelArtifactError::InvalidManifest);
        }
        Ok(labels)
    }

    #[must_use]
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    #[must_use]
    pub fn thresholds(&self) -> &[f32] {
        &self.thresholds
    }

    /// Tokens per window including the bos and eos tokens (`max_length`).
    #[must_use]
    pub const fn window(&self) -> usize {
        self.window
    }

    /// The upstream default window stride: three quarters of the content
    /// tokens, at least one.
    #[must_use]
    pub const fn stride(&self) -> usize {
        let stride = self.window.saturating_sub(2) * 3 / 4;
        if stride == 0 { 1 } else { stride }
    }

    #[must_use]
    pub fn model_sha256(&self) -> Option<&str> {
        self.model_sha256.as_deref()
    }
}

fn sha256_file(path: &Path) -> Result<String, ModelArtifactError> {
    let mut file = File::open(path).map_err(|_| ModelArtifactError::Unreadable)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| ModelArtifactError::Unreadable)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedCompanionEmotionInstall {
    pub manifest: InstalledCompanionEmotionManifest,
    /// Files of an earlier revision could not all be removed yet.
    pub cleanup_pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionEmotionInstallStatus {
    NotInstalled,
    Installed {
        source_revision: String,
    },
    /// A recorded install whose files are missing or have the wrong size.
    Damaged,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompanionEmotionInstallError {
    #[error("the Thymos model description is invalid")]
    InvalidRemote,
    #[error("the installed Thymos model is invalid: {0}")]
    Artifact(#[from] ModelArtifactError),
    #[error("Thymos model storage failed: {0}")]
    Platform(PlatformError),
    #[error("Thymos model files failed: {0}")]
    Pinned(#[from] PinnedArtifactError),
    #[error("a Thymos install is in progress")]
    Busy,
}

impl From<PlatformError> for CompanionEmotionInstallError {
    fn from(error: PlatformError) -> Self {
        Self::Platform(error)
    }
}

/// The managed Thymos root: pinned files below `revisions/<revision>/`, one
/// `installed.json` naming the installed revision and its BLAKE3 identities,
/// and one `active-install.json` naming the install job in progress.
#[derive(Debug)]
pub struct CompanionEmotionInstallStore {
    inner: ConfinedInstallStore,
    root: PathBuf,
}

static INSTALL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Exclusive access to the Thymos root within this process: admission,
/// completion and removal run one at a time.
#[derive(Debug)]
pub struct CompanionEmotionInstallLock<'a> {
    store: &'a CompanionEmotionInstallStore,
    _guard: std::sync::MutexGuard<'static, ()>,
}

/// A hint naming the install job admitted for a revision and the remote it
/// pins, so a later request can join it. Whether any install job runs is the
/// job store's answer, not this record's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveCompanionEmotionInstall {
    pub job_id: String,
    pub remote: RemoteCompanionEmotionModel,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveInstallRecord {
    job_id: String,
    source_revision: String,
    model: ActiveArtifactRecord,
    tokenizer: ActiveArtifactRecord,
    labels: ActiveArtifactRecord,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveArtifactRecord {
    byte_size: u64,
    sha256: Option<String>,
    git_blob_id: Option<String>,
}

impl ActiveArtifactRecord {
    fn from_remote(artifact: &RemoteCompanionEmotionArtifact) -> Self {
        Self {
            byte_size: artifact.byte_size,
            sha256: artifact.sha256.clone(),
            git_blob_id: artifact.git_blob_id.clone(),
        }
    }

    fn into_remote(self, remote_path: &'static str) -> RemoteCompanionEmotionArtifact {
        RemoteCompanionEmotionArtifact {
            remote_path,
            byte_size: self.byte_size,
            sha256: self.sha256,
            git_blob_id: self.git_blob_id,
        }
    }
}

impl CompanionEmotionInstallStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CompanionEmotionInstallError> {
        Ok(Self {
            inner: ConfinedInstallStore::open(root.as_ref())?,
            root: root.as_ref().to_path_buf(),
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn lock(&self) -> CompanionEmotionInstallLock<'_> {
        CompanionEmotionInstallLock {
            store: self,
            _guard: INSTALL_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        }
    }

    /// The recorded install, when its paths are this root's managed paths
    /// for its revision. Contents are verified only when the model loads.
    pub fn installed(
        &self,
    ) -> Result<Option<InstalledCompanionEmotionManifest>, CompanionEmotionInstallError> {
        let Some(bytes) =
            self.read_record(INSTALLED_MANIFEST_FILE, MAX_INSTALLED_MANIFEST_BYTES)?
        else {
            return Ok(None);
        };
        let manifest: InstalledCompanionEmotionManifest =
            serde_json::from_slice(&bytes).map_err(|_| ModelArtifactError::InvalidManifest)?;
        if !is_commit(&manifest.source_revision) {
            return Err(ModelArtifactError::InvalidManifest.into());
        }
        for (artifact, remote_path) in [
            (&manifest.model, THYMOS_MODEL_PATH),
            (&manifest.tokenizer, THYMOS_TOKENIZER_PATH),
            (&manifest.labels, THYMOS_LABELS_PATH),
        ] {
            let key =
                ObjectKey::from_segments(local_segments(&manifest.source_revision, remote_path))?;
            if !self.inner.owns_installed_path(&key, &artifact.path) {
                return Err(ModelArtifactError::InvalidManifest.into());
            }
        }
        Ok(Some(manifest))
    }

    /// Installed when the recorded files exist at their recorded sizes;
    /// loading rehashes them.
    #[must_use]
    pub fn status(&self) -> CompanionEmotionInstallStatus {
        match self.installed() {
            Ok(None) => CompanionEmotionInstallStatus::NotInstalled,
            Ok(Some(manifest)) => {
                let present = [&manifest.model, &manifest.tokenizer, &manifest.labels]
                    .iter()
                    .all(|artifact| {
                        std::fs::symlink_metadata(&artifact.path).is_ok_and(|metadata| {
                            metadata.is_file() && metadata.len() == artifact.byte_size
                        })
                    });
                if present {
                    CompanionEmotionInstallStatus::Installed {
                        source_revision: manifest.source_revision,
                    }
                } else {
                    CompanionEmotionInstallStatus::Damaged
                }
            }
            Err(_) => CompanionEmotionInstallStatus::Damaged,
        }
    }

    fn read_record(
        &self,
        name: &str,
        max_bytes: u64,
    ) -> Result<Option<Vec<u8>>, CompanionEmotionInstallError> {
        let Some(mut file) = self.inner.inspect(&ObjectKey::from_segments([name])?)? else {
            return Ok(None);
        };
        if file.is_empty() || file.len() > max_bytes {
            return Err(ModelArtifactError::InvalidManifest.into());
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|_| ModelArtifactError::Unreadable)?;
        Ok(Some(bytes))
    }

    /// Stages a record beside the current one and renames it over it, so a
    /// crash leaves either the old or the new record.
    fn write_record(&self, name: &str, bytes: &[u8]) -> Result<(), CompanionEmotionInstallError> {
        if u64::try_from(bytes.len()).map_err(|_| ModelArtifactError::InvalidManifest)?
            > MAX_INSTALLED_MANIFEST_BYTES
        {
            return Err(ModelArtifactError::InvalidManifest.into());
        }
        let staged = ObjectKey::from_segments([format!("{name}{STAGED_SUFFIX}")])?;
        let partial =
            ObjectKey::from_segments([DOWNLOADS_DIRECTORY.to_owned(), format!("{name}.part")])?;
        self.inner.discard(&partial)?;
        self.inner.discard(&staged)?;
        let InstallPreparation::Resume(mut install) =
            self.inner
                .prepare(partial, staged, MAX_INSTALLED_MANIFEST_BYTES)?
        else {
            return Err(PlatformError::Conflict.into());
        };
        install.restart()?;
        install.append(bytes)?;
        install.sync()?;
        let staged_path = install.commit()?;
        let target_path = staged_path.with_file_name(name);
        std::fs::rename(&staged_path, &target_path).map_err(|_| PlatformError::ReplaceFailed)?;
        Ok(())
    }

    fn discard_record(&self, name: &str) -> Result<bool, CompanionEmotionInstallError> {
        let staged = self.inner.discard(&ObjectKey::from_segments([format!(
            "{name}{STAGED_SUFFIX}"
        )])?)?;
        Ok(self.inner.discard(&ObjectKey::from_segments([name])?)? || staged)
    }

    /// Removes the three Thymos files of each `revisions/<revision>/`
    /// directory other than `keep`; only paths of the managed layout are
    /// touched. Every revision is attempted before the first error is
    /// returned.
    fn remove_revisions_except(
        &self,
        keep: Option<&str>,
    ) -> Result<bool, CompanionEmotionInstallError> {
        let entries = self.inner.list(
            &ObjectKey::from_segments([REVISIONS_DIRECTORY])?,
            MAX_DIRECTORY_ENTRIES,
        )?;
        let pinned = PinnedArtifactStore::open(&self.root)?;
        let mut removed = false;
        let mut failure = None;
        for entry in entries {
            if entry.is_file || !is_commit(&entry.name) || keep == Some(entry.name.as_str()) {
                continue;
            }
            for remote_path in THYMOS_FILES {
                match pinned.remove(&local_segments(&entry.name, remote_path)) {
                    Ok(true) => removed = true,
                    Ok(false) => {}
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            }
        }
        match failure {
            Some(error) => Err(error.into()),
            None => Ok(removed),
        }
    }

    /// Removes every partial download and staged record below
    /// `.downloads/`; callers hold the lock with no install job running.
    fn remove_partials(&self) -> Result<bool, CompanionEmotionInstallError> {
        let directory = ObjectKey::from_segments([DOWNLOADS_DIRECTORY])?;
        let mut removed = false;
        let mut failure = None;
        for entry in self.inner.list(&directory, MAX_DIRECTORY_ENTRIES)? {
            if !entry.is_file {
                continue;
            }
            match ObjectKey::from_segments([DOWNLOADS_DIRECTORY, entry.name.as_str()])
                .and_then(|key| self.inner.discard(&key))
            {
                Ok(true) => removed = true,
                Ok(false) => {}
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        match failure {
            Some(error) => Err(error.into()),
            None => Ok(removed),
        }
    }
}

impl CompanionEmotionInstallLock<'_> {
    /// The install job recorded as in progress, if any.
    pub fn active(
        &self,
    ) -> Result<Option<ActiveCompanionEmotionInstall>, CompanionEmotionInstallError> {
        let Some(bytes) = self
            .store
            .read_record(ACTIVE_INSTALL_FILE, MAX_INSTALLED_MANIFEST_BYTES)?
        else {
            return Ok(None);
        };
        let record: ActiveInstallRecord =
            serde_json::from_slice(&bytes).map_err(|_| ModelArtifactError::InvalidManifest)?;
        let remote = RemoteCompanionEmotionModel {
            source_revision: record.source_revision,
            model: record.model.into_remote(THYMOS_MODEL_PATH),
            tokenizer: record.tokenizer.into_remote(THYMOS_TOKENIZER_PATH),
            labels: record.labels.into_remote(THYMOS_LABELS_PATH),
        };
        remote.validate()?;
        if record.job_id.trim().is_empty() {
            return Err(ModelArtifactError::InvalidManifest.into());
        }
        Ok(Some(ActiveCompanionEmotionInstall {
            job_id: record.job_id,
            remote,
        }))
    }

    /// Records the admitted install job, replacing any earlier record.
    pub fn record_active(
        &self,
        job_id: &str,
        remote: &RemoteCompanionEmotionModel,
    ) -> Result<(), CompanionEmotionInstallError> {
        remote.validate()?;
        let bytes = serde_json::to_vec(&ActiveInstallRecord {
            job_id: job_id.to_owned(),
            source_revision: remote.source_revision.clone(),
            model: ActiveArtifactRecord::from_remote(&remote.model),
            tokenizer: ActiveArtifactRecord::from_remote(&remote.tokenizer),
            labels: ActiveArtifactRecord::from_remote(&remote.labels),
        })
        .map_err(|_| ModelArtifactError::InvalidManifest)?;
        self.store.write_record(ACTIVE_INSTALL_FILE, &bytes)
    }

    /// Forgets the recorded install job, for one that ended without being
    /// completed.
    pub fn clear_active(&self) -> Result<bool, CompanionEmotionInstallError> {
        self.store.discard_record(ACTIVE_INSTALL_FILE)
    }

    /// Records a finished download: every file must already be in place,
    /// git-stored files must match their git blob id (a mismatching file is
    /// removed so a retry downloads it again), and the full manifest must
    /// verify before it counts as installed. Callers refuse completion while
    /// another install job runs. The record is replaced atomically and the
    /// active install is cleared; the files of every other revision and all
    /// partial downloads are then removed. A failed removal leaves the new
    /// install valid and reports `cleanup_pending`; the next install or
    /// removal sweeps them again.
    pub fn complete(
        &self,
        remote: &RemoteCompanionEmotionModel,
    ) -> Result<CompletedCompanionEmotionInstall, CompanionEmotionInstallError> {
        remote.validate()?;
        let pinned = PinnedArtifactStore::open(&self.store.root)?;
        let mut paths = Vec::with_capacity(3);
        for artifact in remote.artifacts() {
            let path = pinned
                .installed_path(&remote.pinned_artifact(artifact))?
                .ok_or(ModelArtifactError::Missing)?;
            if artifact.sha256.is_none()
                && let Some(blob_id) = artifact.git_blob_id.as_deref()
                && let Err(error) = verify_git_blob(&path, blob_id)
            {
                pinned.remove(&remote.local_segments(artifact))?;
                return Err(error.into());
            }
            paths.push(path);
        }
        let [model, tokenizer, labels] = <[PathBuf; 3]>::try_from(paths)
            .map_err(|_| CompanionEmotionInstallError::InvalidRemote)?;
        let manifest = InstalledCompanionEmotionManifest {
            source_revision: remote.source_revision.clone(),
            model: InstalledModelArtifact::inspect(model)?,
            tokenizer: InstalledModelArtifact::inspect(tokenizer)?,
            labels: InstalledModelArtifact::inspect(labels)?,
        };
        manifest.verify()?;
        let bytes =
            serde_json::to_vec(&manifest).map_err(|_| ModelArtifactError::InvalidManifest)?;
        self.store.write_record(INSTALLED_MANIFEST_FILE, &bytes)?;
        let cleared = self.clear_active().is_ok();
        let revisions = self
            .store
            .remove_revisions_except(Some(&manifest.source_revision))
            .is_ok();
        let partials = self.store.remove_partials().is_ok();
        Ok(CompletedCompanionEmotionInstall {
            manifest,
            cleanup_pending: !(cleared && revisions && partials),
        })
    }

    /// Removes the install: the record first, so an interrupted removal
    /// never leaves a half-deleted model counted as installed, then the
    /// active-install record, the Thymos files of every revision directory
    /// and every partial download. The sweep follows the managed layout
    /// rather than the record, so a corrupt record never strands files and a
    /// failed sweep is retried by the next removal or install. Callers
    /// refuse removal while an install job runs. `false` when nothing was
    /// there.
    pub fn remove(&self) -> Result<bool, CompanionEmotionInstallError> {
        let recorded = self.store.discard_record(INSTALLED_MANIFEST_FILE)?;
        let active = self.clear_active()?;
        let files = self.store.remove_revisions_except(None)?;
        let partials = self.store.remove_partials()?;
        Ok(recorded || active || files || partials)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_types::OperationId;

    use super::*;
    use crate::PinnedArtifactPreparation;

    const REVISION: &str = "57219928da2daf26201012d0aafcc3d58dea74d0";

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn git_blob(bytes: &[u8]) -> String {
        let mut hasher = sha1_smol::Sha1::new();
        hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
        hasher.update(bytes);
        hasher.digest().to_string()
    }

    fn labels_json(model_sha256: Option<&str>) -> Vec<u8> {
        let mut value = serde_json::json!({
            "labels": ["love", "anger", "neutral"],
            "thresholds": [0.43809816241264343, 0.17629921436309814, 0.24908779561519623],
            "max_length": 96,
            "calibration": { "global_threshold": 0.21 }
        });
        if let Some(sha) = model_sha256 {
            value["model_sha256"] = serde_json::Value::String(sha.to_owned());
        }
        serde_json::to_vec(&value).expect("labels")
    }

    fn detail(model: &[u8], tokenizer: &[u8], labels: &[u8]) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "sha": REVISION,
            "siblings": [
                { "rfilename": "README.md", "size": 10 },
                { "rfilename": "onnx/model.onnx", "size": 64, "lfs": { "size": 64, "sha256": "ab".repeat(32) } },
                { "rfilename": THYMOS_MODEL_PATH, "size": model.len(), "lfs": { "size": model.len(), "sha256": sha256(model) } },
                { "rfilename": THYMOS_TOKENIZER_PATH, "size": tokenizer.len(), "lfs": { "size": tokenizer.len(), "sha256": sha256(tokenizer) } },
                { "rfilename": THYMOS_LABELS_PATH, "size": labels.len(), "blobId": git_blob(labels) }
            ]
        }))
        .expect("detail")
    }

    fn install(root: &Path, remote: &RemoteCompanionEmotionModel, files: [&[u8]; 3]) {
        let store = PinnedArtifactStore::open(root).expect("pinned store");
        for (artifact, bytes) in remote.artifacts().into_iter().zip(files) {
            let PinnedArtifactPreparation::Download(mut download) = store
                .prepare(remote.pinned_artifact(artifact))
                .expect("prepare")
            else {
                panic!("expected a download");
            };
            download.append(bytes).expect("append");
            download.finish().expect("finish");
        }
    }

    fn remote_from(
        body: &[u8],
    ) -> Result<RemoteCompanionEmotionModel, CompanionEmotionInstallError> {
        let pinned = crate::pinned_files(THYMOS_REPOSITORY, body, &THYMOS_FILES)
            .map_err(|_| CompanionEmotionInstallError::InvalidRemote)?;
        RemoteCompanionEmotionModel::from_pinned_files(&pinned)
    }

    fn remote(model: &[u8], tokenizer: &[u8], labels: &[u8]) -> RemoteCompanionEmotionModel {
        remote_from(&detail(model, tokenizer, labels)).expect("remote")
    }

    #[test]
    fn the_shared_pin_gives_the_revision_sizes_and_lfs_digests() {
        let labels = labels_json(None);
        let remote = remote(b"model", b"tokenizer", &labels);
        assert_eq!(remote.source_revision, REVISION);
        assert_eq!(remote.model.byte_size, 5);
        assert_eq!(
            remote.model.sha256.as_deref(),
            Some(sha256(b"model").as_str())
        );
        assert_eq!(remote.labels.sha256, None);
        assert_eq!(remote.labels.git_blob_id, Some(git_blob(&labels)));
        assert_eq!(remote.model.git_blob_id, None);
        assert_eq!(remote.total_bytes(), 5 + 9 + labels.len() as u64);
        assert_eq!(
            remote.local_segments(&remote.model),
            ["revisions", REVISION, "onnx", "model_quantized.onnx"]
        );
        assert_eq!(
            remote.source_identity(&remote.tokenizer),
            format!("hf:Zeolit/lettuce-thymos-26m-v1@{REVISION}/tokenizer.json")
        );
    }

    #[test]
    fn a_pin_without_a_commit_or_an_lfs_digest_is_refused() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&detail(b"model", b"tokenizer", b"{}")).expect("value");
        value["sha"] = "main".into();
        assert!(remote_from(&serde_json::to_vec(&value).expect("json")).is_err());
        let mut value: serde_json::Value =
            serde_json::from_slice(&detail(b"model", b"tokenizer", b"{}")).expect("value");
        value["siblings"][2]["lfs"] = serde_json::Value::Null;
        assert_eq!(
            remote_from(&serde_json::to_vec(&value).expect("json")),
            Err(CompanionEmotionInstallError::InvalidRemote)
        );
        let mut value: serde_json::Value =
            serde_json::from_slice(&detail(b"model", b"tokenizer", b"{}")).expect("value");
        value["siblings"][4]
            .as_object_mut()
            .expect("labels")
            .remove("blobId");
        assert_eq!(
            remote_from(&serde_json::to_vec(&value).expect("json")),
            Err(CompanionEmotionInstallError::InvalidRemote),
            "labels.json without a SHA-256 or git blob id"
        );
    }

    #[test]
    fn labels_follow_the_upstream_metadata_rules() {
        let labels = CompanionEmotionLabels::parse(&labels_json(None)).expect("labels");
        assert_eq!(labels.labels(), ["love", "anger", "neutral"]);
        assert_eq!(labels.thresholds()[0], 0.438_098_16_f32);
        assert_eq!(labels.window(), 96);
        assert_eq!(labels.stride(), 70);
        assert_eq!(labels.model_sha256(), None);
        for invalid in [
            serde_json::json!({ "labels": ["a"], "thresholds": [0.5] }),
            serde_json::json!({ "labels": ["a", "b"], "thresholds": [0.5], "max_length": 96 }),
            serde_json::json!({ "labels": ["a"], "thresholds": [1.5], "max_length": 96 }),
            serde_json::json!({ "labels": ["a"], "thresholds": [-0.1], "max_length": 96 }),
            serde_json::json!({ "labels": [], "thresholds": [], "max_length": 96 }),
            serde_json::json!({ "labels": ["a"], "thresholds": [0.5], "max_length": 3 }),
            serde_json::json!({ "labels": ["a"], "thresholds": [0.5], "max_length": 96, "model_sha256": "xyz" }),
        ] {
            assert_eq!(
                CompanionEmotionLabels::parse(&serde_json::to_vec(&invalid).expect("json")),
                Err(ModelArtifactError::InvalidManifest),
                "{invalid}"
            );
        }
        let empty_sha = serde_json::json!({
            "labels": ["a"], "thresholds": [0.5], "max_length": 4, "model_sha256": ""
        });
        let labels = CompanionEmotionLabels::parse(&serde_json::to_vec(&empty_sha).expect("json"))
            .expect("empty digest is absent");
        assert_eq!(labels.model_sha256(), None);
        assert_eq!(labels.stride(), 1);
    }

    #[test]
    fn a_completed_install_is_recorded_verified_and_removed() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(Some(&sha256(b"model")));
        let remote = remote(b"model", b"tokenizer", &labels);
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        assert_eq!(store.status(), CompanionEmotionInstallStatus::NotInstalled);
        assert!(matches!(
            store.lock().complete(&remote),
            Err(CompanionEmotionInstallError::Artifact(
                ModelArtifactError::Missing
            ))
        ));
        install(&root, &remote, [b"model", b"tokenizer", &labels]);
        let manifest = store.lock().complete(&remote).expect("complete").manifest;
        let verified = manifest.verify().expect("verified");
        assert_eq!(verified.labels.window(), 96);
        assert!(verified.model_path.ends_with("onnx/model_quantized.onnx"));
        assert_eq!(store.installed().expect("installed"), Some(manifest));
        assert_eq!(
            store.status(),
            CompanionEmotionInstallStatus::Installed {
                source_revision: REVISION.to_owned()
            }
        );
        assert!(store.lock().remove().expect("remove"));
        assert_eq!(store.status(), CompanionEmotionInstallStatus::NotInstalled);
        assert!(!verified.model_path.exists());
        assert!(!store.lock().remove().expect("removed again"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_model_that_does_not_match_the_labels_digest_is_refused() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(Some(&sha256(b"another model")));
        let remote = remote(b"model", b"tokenizer", &labels);
        install(&root, &remote, [b"model", b"tokenizer", &labels]);
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        assert_eq!(
            store.lock().complete(&remote),
            Err(CompanionEmotionInstallError::Artifact(
                ModelArtifactError::Mismatch
            ))
        );
        assert_eq!(store.status(), CompanionEmotionInstallStatus::NotInstalled);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_tampered_file_fails_verification_and_a_missing_one_is_damaged() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(None);
        let remote = remote(b"model", b"tokenizer", &labels);
        install(&root, &remote, [b"model", b"tokenizer", &labels]);
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        let manifest = store.lock().complete(&remote).expect("complete").manifest;
        std::fs::write(&manifest.tokenizer.path, b"tokenizes").expect("tamper");
        assert_eq!(manifest.verify(), Err(ModelArtifactError::Mismatch));
        std::fs::remove_file(&manifest.model.path).expect("remove model");
        assert_eq!(store.status(), CompanionEmotionInstallStatus::Damaged);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_recorded_path_outside_the_managed_layout_is_refused() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(None);
        let remote = remote(b"model", b"tokenizer", &labels);
        install(&root, &remote, [b"model", b"tokenizer", &labels]);
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        let mut manifest = store.lock().complete(&remote).expect("complete").manifest;
        let outside = std::env::temp_dir().join(format!("thymos-outside-{}", OperationId::new()));
        std::fs::write(&outside, b"model").expect("outside");
        manifest.model.path = outside.clone();
        store
            .write_record(
                INSTALLED_MANIFEST_FILE,
                &serde_json::to_vec(&manifest).expect("json"),
            )
            .expect("write");
        assert_eq!(
            store.installed(),
            Err(CompanionEmotionInstallError::Artifact(
                ModelArtifactError::InvalidManifest
            ))
        );
        assert_eq!(store.status(), CompanionEmotionInstallStatus::Damaged);
        assert!(store.lock().remove().expect("remove"));
        assert!(
            outside.exists(),
            "a path outside the layout is never deleted"
        );
        assert!(
            !root
                .join("revisions")
                .join(REVISION)
                .join("tokenizer.json")
                .exists()
        );
        assert_eq!(store.status(), CompanionEmotionInstallStatus::NotInstalled);
        std::fs::remove_file(outside).ok();
        std::fs::remove_dir_all(root).ok();
    }

    fn other_revision(labels: &[u8]) -> RemoteCompanionEmotionModel {
        let mut value: serde_json::Value =
            serde_json::from_slice(&detail(b"model 2", b"tokenizer 2", labels)).expect("value");
        value["sha"] = "ab".repeat(20).into();
        remote_from(&serde_json::to_vec(&value).expect("json")).expect("remote")
    }

    #[test]
    fn a_new_revision_replaces_the_record_and_sweeps_the_old_files() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(None);
        let first = remote(b"model", b"tokenizer", &labels);
        let second = other_revision(&labels);
        install(&root, &first, [b"model", b"tokenizer", &labels]);
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        let old = store.lock().complete(&first).expect("first").manifest;
        install(&root, &second, [b"model 2", b"tokenizer 2", &labels]);
        let completed = store.lock().complete(&second).expect("second");
        assert!(!completed.cleanup_pending);
        assert_eq!(
            store.installed().expect("record"),
            Some(completed.manifest.clone())
        );
        assert!(!old.model.path.exists());
        assert!(
            !root
                .join(format!("{INSTALLED_MANIFEST_FILE}{STAGED_SUFFIX}"))
                .exists()
        );
        assert!(completed.manifest.verify().is_ok());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_failed_sweep_keeps_the_new_install_and_is_retried() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(None);
        let first = remote(b"model", b"tokenizer", &labels);
        let second = other_revision(&labels);
        install(&root, &second, [b"model 2", b"tokenizer 2", &labels]);
        let stuck = root.join("revisions").join(REVISION).join("tokenizer.json");
        std::fs::create_dir_all(&stuck).expect("unremovable entry");
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        let completed = store.lock().complete(&second).expect("install stays valid");
        assert!(completed.cleanup_pending);
        assert_eq!(
            store.status(),
            CompanionEmotionInstallStatus::Installed {
                source_revision: second.source_revision.clone()
            }
        );
        std::fs::remove_dir(&stuck).expect("unstick");
        install(&root, &first, [b"model", b"tokenizer", &labels]);
        let completed = store.lock().complete(&second).expect("again");
        assert!(!completed.cleanup_pending);
        assert!(
            !root
                .join("revisions")
                .join(REVISION)
                .join("labels.json")
                .exists()
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_corrupt_record_is_removed_with_every_revision_in_the_layout() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(None);
        let remote = remote(b"model", b"tokenizer", &labels);
        install(&root, &remote, [b"model", b"tokenizer", &labels]);
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        store.lock().complete(&remote).expect("complete");
        std::fs::write(root.join(INSTALLED_MANIFEST_FILE), b"{not json").expect("corrupt");
        assert_eq!(store.status(), CompanionEmotionInstallStatus::Damaged);
        assert!(store.lock().remove().expect("remove"));
        assert_eq!(store.status(), CompanionEmotionInstallStatus::NotInstalled);
        for remote_path in THYMOS_FILES {
            assert!(
                !root
                    .join("revisions")
                    .join(REVISION)
                    .join(remote_path)
                    .exists()
            );
        }
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn labels_that_do_not_match_their_git_blob_are_refused_and_dropped() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(None);
        let remote = remote(b"model", b"tokenizer", &labels);
        let mut tampered = labels.clone();
        let last = tampered.len() - 1;
        tampered[last - 1] = b' ';
        install(&root, &remote, [b"model", b"tokenizer", &tampered]);
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        assert_eq!(
            store.lock().complete(&remote),
            Err(CompanionEmotionInstallError::Pinned(
                PinnedArtifactError::Mismatch
            ))
        );
        assert!(
            !root
                .join("revisions")
                .join(REVISION)
                .join("labels.json")
                .exists()
        );
        assert_eq!(store.status(), CompanionEmotionInstallStatus::NotInstalled);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn the_active_install_hint_is_recorded_and_cleared_on_completion() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(None);
        let first = remote(b"model", b"tokenizer", &labels);
        let second = other_revision(&labels);
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        let lock = store.lock();
        assert_eq!(lock.active().expect("none"), None);
        lock.record_active("job-2", &second).expect("record");
        assert_eq!(
            lock.active().expect("active"),
            Some(ActiveCompanionEmotionInstall {
                job_id: "job-2".to_owned(),
                remote: second.clone(),
            })
        );
        install(&root, &first, [b"model", b"tokenizer", &labels]);
        lock.complete(&first).expect("first");
        install(&root, &second, [b"model 2", b"tokenizer 2", &labels]);
        let completed = lock.complete(&second).expect("complete");
        assert!(!completed.cleanup_pending);
        assert_eq!(lock.active().expect("cleared"), None);
        assert!(
            !root
                .join("revisions")
                .join(REVISION)
                .join("tokenizer.json")
                .exists()
        );
        drop(lock);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn completion_and_removal_sweep_partial_downloads() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(None);
        let remote = remote(b"model", b"tokenizer", &labels);
        install(&root, &remote, [b"model", b"tokenizer", &labels]);
        let partial = root.join(DOWNLOADS_DIRECTORY).join("stale.part");
        std::fs::write(&partial, b"partial").expect("partial");
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        store.lock().complete(&remote).expect("complete");
        assert!(!partial.exists());
        std::fs::write(&partial, b"partial").expect("partial again");
        assert!(store.lock().remove().expect("remove"));
        assert!(!partial.exists());
        assert!(!store.lock().remove().expect("nothing left"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn completion_and_removal_are_serialized() {
        let root = std::env::temp_dir().join(format!("thymos-{}", OperationId::new()));
        let labels = labels_json(None);
        let remote = remote(b"model", b"tokenizer", &labels);
        install(&root, &remote, [b"model", b"tokenizer", &labels]);
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        let held = store.lock();
        let (sender, receiver) = std::sync::mpsc::channel();
        let remover = {
            let root = root.clone();
            std::thread::spawn(move || {
                let store = CompanionEmotionInstallStore::open(&root).expect("store");
                let removed = store.lock().remove().expect("remove");
                sender.send(removed).expect("send");
            })
        };
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_millis(200))
                .is_err(),
            "removal waits for the held lock"
        );
        let completed = held.complete(&remote).expect("complete");
        drop(held);
        assert!(receiver.recv().expect("removed"));
        remover.join().expect("remover");
        assert!(!completed.manifest.model.path.exists());
        assert_eq!(store.status(), CompanionEmotionInstallStatus::NotInstalled);
        std::fs::remove_dir_all(root).ok();
    }
}
