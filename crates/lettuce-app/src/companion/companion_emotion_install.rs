//! The Lettuce Thymos companion emotion model: resolved from the Hugging Face
//! API and pinned to the commit it reports, installed as one resumable
//! `ArtifactInstall` job (progress and cancellation come from the job), then
//! recorded, reported, loaded and removed through its managed root. Only
//! one Thymos install is admitted at a time, whatever revision it pins.

use std::path::{Path, PathBuf};

use lettuce_embeddings::OnnxRuntimeLink;
use lettuce_jobs::{JobKind, JobQuery, JobSnapshot, JobState, JobStore, SubjectKind};
use lettuce_model_hub::{
    CompanionEmotionInstallError, CompanionEmotionInstallLock, CompanionEmotionInstallStatus,
    CompanionEmotionInstallStore, HfBrowseError, HfResource, InstalledCompanionEmotionManifest,
    RemoteCompanionEmotionModel, THYMOS_FILES, THYMOS_REPOSITORY,
};
use lettuce_network::JsonClientError;
use lettuce_types::{JobId, PageRequest};

use crate::{
    ArtifactInstallCoordinator, ArtifactInstallError, ArtifactInstallPlan, ArtifactSource,
    CompanionEmotionService, CompanionEmotionServiceError, HuggingFaceBrowser, PlannedArtifact,
};

/// `<app folder>/models/thymos`. Files under
/// `models/embedding/companion-emotion` belong to a different model and are
/// never read, moved or deleted.
#[must_use]
pub fn companion_emotion_root(app_folder: &Path) -> PathBuf {
    app_folder.join("models").join("thymos")
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionEmotionDownloadError {
    #[error("the Thymos model lookup failed: {0}")]
    Network(JsonClientError),
    #[error("the Thymos model lookup returned status {0}")]
    Response(u16),
    #[error("{0}")]
    Browse(#[from] HfBrowseError),
    #[error("{0}")]
    Install(#[from] CompanionEmotionInstallError),
    #[error("{0}")]
    Jobs(#[from] ArtifactInstallError),
    #[error("a Thymos install is in progress")]
    InstallInProgress(Box<JobSnapshot>),
}

impl HuggingFaceBrowser {
    /// The current Thymos files, pinned through the shared model pin to the
    /// commit Hugging Face reports. The repository is public, so no token
    /// is sent.
    pub async fn companion_emotion_model(
        &self,
    ) -> Result<RemoteCompanionEmotionModel, CompanionEmotionDownloadError> {
        let response = self
            .get(
                &lettuce_model_hub::model_pin_request(THYMOS_REPOSITORY),
                None,
            )
            .await
            .map_err(CompanionEmotionDownloadError::Network)?;
        if let Some(error) = lettuce_model_hub::access_error(
            response.status,
            HfResource::Model,
            THYMOS_REPOSITORY,
            false,
        ) {
            return Err(error.into());
        }
        if response.status != 200 {
            return Err(CompanionEmotionDownloadError::Response(response.status));
        }
        let pinned =
            lettuce_model_hub::pinned_files(THYMOS_REPOSITORY, &response.body, &THYMOS_FILES)?;
        Ok(RemoteCompanionEmotionModel::from_pinned_files(&pinned)?)
    }
}

/// The display label every Thymos install job carries, which is how the job
/// store answers whether a Thymos install (of any revision) is active.
pub const COMPANION_EMOTION_JOB_LABEL: &str = "Lettuce Thymos";

/// An admitted Thymos install: the job to claim and run (or to wait on,
/// when it was admitted earlier) with the revision and plan it installs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionEmotionAdmission {
    pub remote: RemoteCompanionEmotionModel,
    pub plan: ArtifactInstallPlan,
    pub job: JobSnapshot,
    pub created: bool,
}

/// Admits an install of `remote`. While a Thymos install job is queued or
/// running (any revision), that job is returned instead (`created ==
/// false`) with the revision and plan its hint recorded; when the hint
/// cannot supply them, the request is refused with
/// [`CompanionEmotionDownloadError::InstallInProgress`] carrying the job to
/// wait on. Two revisions therefore never download or complete at once.
pub fn admit_companion_emotion_install<J: JobStore + ?Sized>(
    jobs: &J,
    root: &Path,
    remote: &RemoteCompanionEmotionModel,
) -> Result<CompanionEmotionAdmission, CompanionEmotionDownloadError> {
    let store = CompanionEmotionInstallStore::open(root)?;
    let lock = store.lock();
    reconcile(jobs, &lock)?;
    if let Some(job) = active_job(jobs)? {
        let hint = lock.active().ok().flatten();
        return match hint {
            Some(hint) if hint.job_id == job.id.to_string() => Ok(CompanionEmotionAdmission {
                plan: install_plan(root, &hint.remote)?,
                remote: hint.remote,
                job,
                created: false,
            }),
            _ => Err(CompanionEmotionDownloadError::InstallInProgress(Box::new(
                job,
            ))),
        };
    }
    lock.clear_active()?;
    let plan = install_plan(root, remote)?;
    let admitted =
        ArtifactInstallCoordinator::new(jobs).admit_labeled(&plan, COMPANION_EMOTION_JOB_LABEL)?;
    lock.record_active(&admitted.job.id.to_string(), remote)?;
    Ok(CompanionEmotionAdmission {
        remote: remote.clone(),
        plan,
        job: admitted.job,
        created: admitted.created,
    })
}

/// Every Thymos install job the job store knows, found by its label.
fn thymos_jobs<J: JobStore + ?Sized>(
    jobs: &J,
) -> Result<Vec<JobSnapshot>, CompanionEmotionDownloadError> {
    let mut found = Vec::new();
    let mut page = PageRequest::default();
    loop {
        let listed = jobs
            .list(JobQuery {
                kind: Some(JobKind::ArtifactInstall),
                page,
                ..JobQuery::default()
            })
            .map_err(|error| CompanionEmotionDownloadError::Jobs(error.into()))?;
        found.extend(listed.items.into_iter().filter(|job| {
            job.subject.kind == SubjectKind::ArtifactInstall
                && job
                    .subject
                    .display
                    .as_ref()
                    .is_some_and(|display| display.as_str() == COMPANION_EMOTION_JOB_LABEL)
        }));
        match listed.next_cursor {
            Some(cursor) => {
                page = PageRequest {
                    cursor: Some(cursor),
                    ..PageRequest::default()
                };
            }
            None => return Ok(found),
        }
    }
}

/// The queued or running Thymos install job, if any.
fn active_job<J: JobStore + ?Sized>(
    jobs: &J,
) -> Result<Option<JobSnapshot>, CompanionEmotionDownloadError> {
    Ok(thymos_jobs(jobs)?
        .into_iter()
        .filter(|job| !job.state.is_terminal())
        .max_by_key(|job| job.created_at))
}

/// Settles the hint against the job store: a hint whose job succeeded is
/// finished as [`finish_companion_emotion_install`] would (a failed
/// verification is logged and leaves the model not installed by that job);
/// a hint whose job ended otherwise, no longer exists, or cannot be read
/// while no Thymos job is active is discarded. Idempotent.
fn reconcile<J: JobStore + ?Sized>(
    jobs: &J,
    lock: &CompanionEmotionInstallLock<'_>,
) -> Result<(), CompanionEmotionDownloadError> {
    let hint = match lock.active() {
        Ok(Some(hint)) => hint,
        Ok(None) => return Ok(()),
        Err(_) => {
            if active_job(jobs)?.is_none() {
                lock.clear_active()?;
            }
            return Ok(());
        }
    };
    let job = match hint.job_id.parse::<JobId>() {
        Ok(job_id) => jobs
            .get(job_id)
            .map_err(|error| CompanionEmotionDownloadError::Jobs(error.into()))?,
        Err(_) => None,
    };
    match job.map(|job| job.state) {
        Some(state) if !state.is_terminal() => {}
        Some(JobState::Succeeded) => {
            if let Err(error) = complete(lock, &hint.remote) {
                tracing::warn!(
                    %error,
                    revision = %hint.remote.source_revision,
                    "a downloaded Thymos install failed verification and was not recorded"
                );
                lock.clear_active()?;
            }
        }
        _ => {
            lock.clear_active()?;
        }
    }
    Ok(())
}

fn install_plan(
    root: &Path,
    remote: &RemoteCompanionEmotionModel,
) -> Result<ArtifactInstallPlan, CompanionEmotionDownloadError> {
    remote.validate()?;
    let artifacts = remote
        .artifacts()
        .into_iter()
        .map(|artifact| {
            let source = ArtifactSource::HuggingFace {
                repository: THYMOS_REPOSITORY.to_owned(),
                revision: remote.source_revision.clone(),
                path: artifact.remote_path.to_owned(),
            };
            let pinned = remote.pinned_artifact(artifact);
            debug_assert_eq!(pinned.source_identity, source.identity());
            PlannedArtifact {
                source,
                artifact: pinned,
            }
        })
        .collect();
    Ok(ArtifactInstallPlan {
        install_id: format!("thymos:{THYMOS_REPOSITORY}@{}", remote.source_revision),
        root: root.to_path_buf(),
        artifacts,
    })
}

/// Records a succeeded install after verifying every file, the label
/// metadata and its optional model digest, then sweeps other revisions and
/// partial downloads. Files that could not be removed are logged and swept
/// again by the next install or removal; the new install stands. Refused as
/// busy while a Thymos install job is queued or running.
pub fn finish_companion_emotion_install<J: JobStore + ?Sized>(
    jobs: &J,
    root: &Path,
    remote: &RemoteCompanionEmotionModel,
) -> Result<InstalledCompanionEmotionManifest, CompanionEmotionDownloadError> {
    let store = CompanionEmotionInstallStore::open(root)?;
    let lock = store.lock();
    if active_job(jobs)?.is_some() {
        return Err(CompanionEmotionInstallError::Busy.into());
    }
    Ok(complete(&lock, remote)?)
}

fn complete(
    lock: &CompanionEmotionInstallLock<'_>,
    remote: &RemoteCompanionEmotionModel,
) -> Result<InstalledCompanionEmotionManifest, CompanionEmotionInstallError> {
    let completed = lock.complete(remote)?;
    if completed.cleanup_pending {
        tracing::warn!(
            revision = %completed.manifest.source_revision,
            "Thymos cleanup is incomplete; the next install or removal retries"
        );
    }
    Ok(completed.manifest)
}

/// The install state after settling a succeeded but unrecorded install.
pub fn companion_emotion_status<J: JobStore + ?Sized>(
    jobs: &J,
    root: &Path,
) -> Result<CompanionEmotionInstallStatus, CompanionEmotionDownloadError> {
    let store = CompanionEmotionInstallStore::open(root)?;
    reconcile(jobs, &store.lock())?;
    Ok(store.status())
}

/// Refused as busy while a Thymos install job is queued or running; its
/// files are left alone. `false` when no Thymos record or files were there.
pub fn remove_companion_emotion<J: JobStore + ?Sized>(
    jobs: &J,
    root: &Path,
) -> Result<bool, CompanionEmotionDownloadError> {
    let store = CompanionEmotionInstallStore::open(root)?;
    let lock = store.lock();
    if active_job(jobs)?.is_some() {
        return Err(CompanionEmotionInstallError::Busy.into());
    }
    Ok(lock.remove()?)
}

/// The classifier of the recorded install. `None` when Thymos is not
/// installed or cannot be used (a damaged install, a failed verification or
/// no ONNX Runtime), which is logged; companion turns then take the neutral
/// update.
#[must_use]
pub fn load_companion_emotion(
    root: &Path,
    runtime_link: &OnnxRuntimeLink,
) -> Option<CompanionEmotionService> {
    match try_load_companion_emotion(root, runtime_link) {
        Ok(service) => service,
        Err(error) => {
            tracing::warn!(%error, "companion emotion classifier unavailable; using neutral updates");
            None
        }
    }
}

/// [`load_companion_emotion`] with the reason a recorded install cannot be
/// used, for status reporting.
pub fn try_load_companion_emotion(
    root: &Path,
    runtime_link: &OnnxRuntimeLink,
) -> Result<Option<CompanionEmotionService>, CompanionEmotionServiceError> {
    let Some(manifest) = CompanionEmotionInstallStore::open(root)?.installed()? else {
        return Ok(None);
    };
    CompanionEmotionService::load(&manifest, runtime_link).map(Some)
}

#[cfg(test)]
mod tests {
    use std::{sync::Mutex, time::Duration};

    use async_trait::async_trait;
    use lettuce_database::Database;
    use lettuce_jobs::{CancellationReason, JobState, ResourceAvailability, WorkerId};
    use lettuce_types::{OperationId, TimestampMillis};
    use sha2::{Digest, Sha256};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;
    use crate::{
        ArtifactBody, ArtifactInstallCoordinator, ArtifactInstallRunResult, ArtifactSourceClient,
        ArtifactSourceError,
    };

    const REVISION: &str = "57219928da2daf26201012d0aafcc3d58dea74d0";

    const LABELS: &[u8] = br#"{"labels":["love","neutral"],"thresholds":[0.43809816241264343,0.24908779561519623],"max_length":96}"#;
    const LABELS_GIT_BLOB: &str = "b08f6ff5845b296024e3f71acefb7f270a4a67b9";

    fn labels() -> Vec<u8> {
        LABELS.to_vec()
    }

    fn detail(files: &[(&str, &[u8])]) -> String {
        let siblings = files
            .iter()
            .map(|(path, bytes)| {
                if path.ends_with(".json") && *path != "tokenizer.json" {
                    assert_eq!(*bytes, LABELS);
                    serde_json::json!({ "rfilename": path, "size": bytes.len(), "blobId": LABELS_GIT_BLOB })
                } else {
                    serde_json::json!({
                        "rfilename": path,
                        "size": bytes.len(),
                        "lfs": { "size": bytes.len(), "sha256": format!("{:x}", Sha256::digest(bytes)) }
                    })
                }
            })
            .collect::<Vec<_>>();
        serde_json::json!({ "sha": REVISION, "siblings": siblings }).to_string()
    }

    struct Body {
        bytes: Vec<u8>,
        start: u64,
        sent: bool,
    }

    #[async_trait]
    impl ArtifactBody for Body {
        fn start(&self) -> u64 {
            self.start
        }

        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ArtifactSourceError> {
            if self.sent {
                return Ok(None);
            }
            self.sent = true;
            Ok(Some(
                self.bytes[usize::try_from(self.start).expect("offset")..].to_vec(),
            ))
        }
    }

    struct Source {
        files: Vec<(String, Vec<u8>)>,
        opened: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl ArtifactSourceClient for Source {
        async fn open(
            &self,
            source: &ArtifactSource,
            offset: u64,
            _expected_size: u64,
        ) -> Result<Box<dyn ArtifactBody>, ArtifactSourceError> {
            let ArtifactSource::HuggingFace {
                repository,
                revision,
                path,
            } = source
            else {
                return Err(ArtifactSourceError::InvalidRequest);
            };
            assert_eq!(repository, THYMOS_REPOSITORY);
            assert_eq!(revision, REVISION);
            self.opened.lock().expect("opened").push(path.clone());
            let (_, bytes) = self
                .files
                .iter()
                .find(|(candidate, _)| candidate == path)
                .ok_or(ArtifactSourceError::InvalidRequest)?;
            Ok(Box::new(Body {
                bytes: bytes.clone(),
                start: offset,
                sent: false,
            }))
        }
    }

    async fn serve_once(body: String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).await.expect("read");
            assert!(
                String::from_utf8_lossy(&buffer[..read])
                    .starts_with("GET /api/models/Zeolit/lettuce-thymos-26m-v1?blobs=true ")
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.expect("write");
        });
        format!("http://{address}")
    }

    async fn run_install(
        database: &Database,
        root: &Path,
        remote: &RemoteCompanionEmotionModel,
        source: &Source,
        cancel: bool,
    ) -> ArtifactInstallRunResult {
        let admission = admit_companion_emotion_install(database, root, remote).expect("admit");
        let coordinator = ArtifactInstallCoordinator::new(database);
        let work = coordinator
            .claim(
                admission.plan.clone(),
                admission.job.id,
                WorkerId::new(),
                TimestampMillis::new(1),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        if cancel {
            work.handle.request_cancel();
        }
        coordinator
            .run(
                work,
                source,
                CancellationReason::User,
                TimestampMillis::new(2),
            )
            .await
            .expect("run")
    }

    fn files(labels: &[u8]) -> [(&'static str, Vec<u8>); 3] {
        [
            ("onnx/model_quantized.onnx", b"model bytes".to_vec()),
            ("tokenizer.json", b"tokenizer bytes".to_vec()),
            ("labels.json", labels.to_vec()),
        ]
    }

    fn source(files: &[(&'static str, Vec<u8>); 3]) -> Source {
        Source {
            files: files
                .iter()
                .map(|(path, bytes)| ((*path).to_owned(), bytes.clone()))
                .collect(),
            opened: Mutex::new(Vec::new()),
        }
    }

    fn pinned(files: &[(&'static str, Vec<u8>); 3], revision: &str) -> RemoteCompanionEmotionModel {
        let listed = files
            .iter()
            .map(|(path, bytes)| (*path, bytes.as_slice()))
            .collect::<Vec<_>>();
        let body = detail(&listed).replace(REVISION, revision);
        let pinned =
            lettuce_model_hub::pinned_files(THYMOS_REPOSITORY, body.as_bytes(), &THYMOS_FILES)
                .expect("pin");
        RemoteCompanionEmotionModel::from_pinned_files(&pinned).expect("remote")
    }

    #[tokio::test]
    async fn thymos_is_pinned_installed_recorded_and_removed() {
        let root = std::env::temp_dir().join(format!("thymos-install-{}", OperationId::new()));
        let files = files(&labels());
        let listed = files
            .iter()
            .map(|(path, bytes)| (*path, bytes.as_slice()))
            .collect::<Vec<_>>();
        let endpoint = serve_once(detail(&listed)).await;
        let remote = HuggingFaceBrowser::with_endpoint(
            lettuce_network::JsonClient::new().expect("client"),
            endpoint,
        )
        .companion_emotion_model()
        .await
        .expect("remote");
        assert_eq!(remote.source_revision, REVISION);
        let database = Database::open_in_memory().expect("database");
        let admission =
            admit_companion_emotion_install(&database, &root, &remote).expect("admission");
        assert!(admission.created);
        assert_eq!(
            admission.plan.install_id,
            format!("thymos:Zeolit/lettuce-thymos-26m-v1@{REVISION}")
        );
        assert_eq!(admission.plan.total_bytes(), remote.total_bytes());
        assert_eq!(
            admission.plan.artifacts[0].artifact.local_segments,
            ["revisions", REVISION, "onnx", "model_quantized.onnx"]
        );
        assert_eq!(
            companion_emotion_status(&database, &root).expect("status"),
            CompanionEmotionInstallStatus::NotInstalled
        );

        let source = source(&files);
        let cancelled = run_install(&database, &root, &remote, &source, true).await;
        assert!(matches!(
            cancelled,
            ArtifactInstallRunResult::Cancelled { ref job } if job.state == JobState::Cancelled
        ));
        assert!(source.opened.lock().expect("opened").is_empty());

        let installed = run_install(&database, &root, &remote, &source, false).await;
        assert!(matches!(
            installed,
            ArtifactInstallRunResult::Succeeded { .. }
        ));
        assert_eq!(
            CompanionEmotionInstallStore::open(&root)
                .expect("store")
                .status(),
            CompanionEmotionInstallStatus::NotInstalled,
            "downloaded files count only once recorded"
        );
        let manifest = finish_companion_emotion_install(&database, &root, &remote).expect("finish");
        assert_eq!(manifest.source_revision, REVISION);
        assert_eq!(
            companion_emotion_status(&database, &root).expect("status"),
            CompanionEmotionInstallStatus::Installed {
                source_revision: REVISION.to_owned()
            }
        );
        let verified = manifest.verify().expect("verified");
        assert_eq!(verified.labels.labels(), ["love", "neutral"]);

        assert!(remove_companion_emotion(&database, &root).expect("remove"));
        assert_eq!(
            companion_emotion_status(&database, &root).expect("status"),
            CompanionEmotionInstallStatus::NotInstalled
        );
        assert!(!verified.model_path.exists());
        assert!(load_companion_emotion(&root, &OnnxRuntimeLink::Linked).is_none());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_second_revision_joins_the_active_install() {
        let root = std::env::temp_dir().join(format!("thymos-join-{}", OperationId::new()));
        let files = files(&labels());
        let first = pinned(&files, REVISION);
        let second = pinned(&files, &"ab".repeat(20));
        let database = Database::open_in_memory().expect("database");
        let admitted = admit_companion_emotion_install(&database, &root, &first).expect("first");
        assert!(admitted.created);
        let joined = admit_companion_emotion_install(&database, &root, &second).expect("second");
        assert!(!joined.created);
        assert_eq!(joined.job.id, admitted.job.id);
        assert_eq!(joined.remote, first);
        assert_eq!(joined.plan, admitted.plan);
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn removal_is_refused_while_an_install_is_active() {
        let root = std::env::temp_dir().join(format!("thymos-busy-{}", OperationId::new()));
        let files = files(&labels());
        let remote = pinned(&files, REVISION);
        let database = Database::open_in_memory().expect("database");
        run_install(&database, &root, &remote, &source(&files), false).await;
        finish_companion_emotion_install(&database, &root, &remote).expect("finish");
        let second = pinned(&files, &"ab".repeat(20));
        admit_companion_emotion_install(&database, &root, &second).expect("queued");
        assert!(matches!(
            remove_companion_emotion(&database, &root),
            Err(CompanionEmotionDownloadError::Install(
                CompanionEmotionInstallError::Busy
            ))
        ));
        assert_eq!(
            companion_emotion_status(&database, &root).expect("status"),
            CompanionEmotionInstallStatus::Installed {
                source_revision: REVISION.to_owned()
            }
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn a_damaged_install_loads_as_unavailable() {
        let root = std::env::temp_dir().join(format!("thymos-damaged-{}", OperationId::new()));
        let files = files(&labels());
        let remote = pinned(&files, REVISION);
        let database = Database::open_in_memory().expect("database");
        run_install(&database, &root, &remote, &source(&files), false).await;
        let manifest = finish_companion_emotion_install(&database, &root, &remote).expect("finish");
        std::fs::write(&manifest.tokenizer.path, b"tokenizer bytez").expect("tamper");
        assert!(matches!(
            try_load_companion_emotion(&root, &OnnxRuntimeLink::Linked),
            Err(CompanionEmotionServiceError::Artifacts(_))
        ));
        assert!(load_companion_emotion(&root, &OnnxRuntimeLink::Linked).is_none());
        std::fs::write(root.join("installed.json"), b"{broken").expect("corrupt");
        assert!(try_load_companion_emotion(&root, &OnnxRuntimeLink::Linked).is_err());
        assert!(load_companion_emotion(&root, &OnnxRuntimeLink::Linked).is_none());
        std::fs::remove_dir_all(root).ok();
    }

    fn write_hint(root: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(root).expect("root");
        std::fs::write(root.join("active-install.json"), bytes).expect("hint");
    }

    #[test]
    fn a_thymos_job_without_a_hint_is_waited_on_and_blocks_removal() {
        let root = std::env::temp_dir().join(format!("thymos-nohint-{}", OperationId::new()));
        let files = files(&labels());
        let first = pinned(&files, REVISION);
        let database = Database::open_in_memory().expect("database");
        let job = ArtifactInstallCoordinator::new(&database)
            .admit_labeled(
                &install_plan(&root, &first).expect("plan"),
                COMPANION_EMOTION_JOB_LABEL,
            )
            .expect("job")
            .job;
        let second = pinned(&files, &"ab".repeat(20));
        match admit_companion_emotion_install(&database, &root, &second) {
            Err(CompanionEmotionDownloadError::InstallInProgress(active)) => {
                assert_eq!(active.id, job.id);
            }
            other => panic!("expected the active job: {other:?}"),
        }
        assert!(matches!(
            remove_companion_emotion(&database, &root),
            Err(CompanionEmotionDownloadError::Install(
                CompanionEmotionInstallError::Busy
            ))
        ));
        assert!(matches!(
            finish_companion_emotion_install(&database, &root, &first),
            Err(CompanionEmotionDownloadError::Install(
                CompanionEmotionInstallError::Busy
            ))
        ));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_corrupt_hint_without_an_active_job_is_discarded() {
        let root = std::env::temp_dir().join(format!("thymos-hint-{}", OperationId::new()));
        let files = files(&labels());
        let remote = pinned(&files, REVISION);
        let database = Database::open_in_memory().expect("database");
        write_hint(&root, b"{broken");
        let admission =
            admit_companion_emotion_install(&database, &root, &remote).expect("admission");
        assert!(admission.created);
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        let hint = store.lock().active().expect("readable").expect("hint");
        assert_eq!(hint.job_id, admission.job.id.to_string());
        assert_eq!(hint.remote, remote);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_corrupt_hint_with_an_active_job_is_busy() {
        let root = std::env::temp_dir().join(format!("thymos-busyhint-{}", OperationId::new()));
        let files = files(&labels());
        let remote = pinned(&files, REVISION);
        let database = Database::open_in_memory().expect("database");
        let admission =
            admit_companion_emotion_install(&database, &root, &remote).expect("admission");
        write_hint(&root, b"{broken");
        assert!(matches!(
            admit_companion_emotion_install(&database, &root, &remote),
            Err(CompanionEmotionDownloadError::InstallInProgress(job)) if job.id == admission.job.id
        ));
        assert!(matches!(
            remove_companion_emotion(&database, &root),
            Err(CompanionEmotionDownloadError::Install(
                CompanionEmotionInstallError::Busy
            ))
        ));
        assert!(matches!(
            finish_companion_emotion_install(&database, &root, &remote),
            Err(CompanionEmotionDownloadError::Install(
                CompanionEmotionInstallError::Busy
            ))
        ));
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn status_records_a_succeeded_install_that_was_never_finished() {
        let root = std::env::temp_dir().join(format!("thymos-reconcile-{}", OperationId::new()));
        let files = files(&labels());
        let remote = pinned(&files, REVISION);
        let database = Database::open_in_memory().expect("database");
        let result = run_install(&database, &root, &remote, &source(&files), false).await;
        assert!(matches!(result, ArtifactInstallRunResult::Succeeded { .. }));
        assert_eq!(
            companion_emotion_status(&database, &root).expect("status"),
            CompanionEmotionInstallStatus::Installed {
                source_revision: REVISION.to_owned()
            }
        );
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        assert_eq!(store.lock().active().expect("hint"), None);
        assert_eq!(
            companion_emotion_status(&database, &root).expect("again"),
            CompanionEmotionInstallStatus::Installed {
                source_revision: REVISION.to_owned()
            }
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn a_succeeded_install_that_fails_verification_stays_uninstalled() {
        let root = std::env::temp_dir().join(format!("thymos-badjob-{}", OperationId::new()));
        let files = files(&labels());
        let remote = pinned(&files, REVISION);
        let database = Database::open_in_memory().expect("database");
        run_install(&database, &root, &remote, &source(&files), false).await;
        std::fs::write(
            root.join("revisions").join(REVISION).join("labels.json"),
            vec![b' '; LABELS.len()],
        )
        .expect("tamper");
        assert_eq!(
            companion_emotion_status(&database, &root).expect("status"),
            CompanionEmotionInstallStatus::NotInstalled
        );
        let store = CompanionEmotionInstallStore::open(&root).expect("store");
        assert_eq!(store.lock().active().expect("hint"), None);
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    #[ignore = "downloads Lettuce Thymos from Hugging Face and needs an ONNX Runtime library"]
    async fn live_thymos_download_classifies_text() {
        use crate::CompanionEmotionEngine;

        let runtime = std::env::var_os("LETTUCE_TEST_ONNX_RUNTIME")
            .map(PathBuf::from)
            .expect("runtime path");
        let root = std::env::var_os("LETTUCE_TEST_THYMOS_ROOT").map_or_else(
            || std::env::temp_dir().join(format!("thymos-live-{}", OperationId::new())),
            PathBuf::from,
        );
        let remote = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"))
            .companion_emotion_model()
            .await
            .expect("remote");
        let database = Database::open_in_memory().expect("database");
        let coordinator = ArtifactInstallCoordinator::new(&database);
        let admitted = admit_companion_emotion_install(&database, &root, &remote).expect("admit");
        let plan = admitted.plan.clone();
        let work = coordinator
            .claim(
                plan.clone(),
                admitted.job.id,
                WorkerId::new(),
                TimestampMillis::new(1),
                Duration::from_secs(600),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        let result = coordinator
            .run(
                work,
                &lettuce_network::ArtifactDownloadClient::new().expect("client"),
                CancellationReason::User,
                TimestampMillis::new(2),
            )
            .await
            .expect("install");
        assert!(
            matches!(result, ArtifactInstallRunResult::Succeeded { .. }),
            "{result:?}"
        );
        finish_companion_emotion_install(&database, &root, &remote).expect("finish");
        let service = try_load_companion_emotion(&root, &OnnxRuntimeLink::Dynamic(runtime))
            .expect("load")
            .expect("installed");
        let classification = service
            .classify_emotion(
                "I love spending time with you.",
                &lettuce_jobs::handle::CancellationToken::new(),
            )
            .expect("classify")
            .expect("nonblank");
        assert_eq!(classification.labels.len(), 28);
        assert_eq!(classification.labels[0].label, "love");
        let bundle = lettuce_companions::signals_from_classification(&classification);
        assert!(bundle.signals.iter().any(|signal| signal == "emotion:love"));
    }
}
