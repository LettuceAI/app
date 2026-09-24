use std::sync::Mutex;

use lettuce_database::Database;
use lettuce_embeddings::{CommittedOnnxRuntime, OnnxRuntimeSource};
use lettuce_types::OperationId;

use super::*;
use crate::ArtifactBody;

struct TempApp(PathBuf);

impl TempApp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("lettuce-ort-app-{}", OperationId::new()));
        std::fs::create_dir_all(&path).expect("app dir");
        Self(path)
    }

    fn paths(&self) -> OnnxRuntimePaths {
        OnnxRuntimePaths::legacy_layout(&self.0, None)
    }
}

impl Drop for TempApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn no_events(_: OnnxRuntimeInstallEvent) {}

#[test]
fn legacy_layout_keeps_the_runtime_below_the_app_folder() {
    let paths = OnnxRuntimePaths::legacy_layout(Path::new("/app/lettuce"), None);
    assert_eq!(paths.install_dir, Path::new("/app/lettuce/onnxruntime"));
    assert_eq!(
        paths.downloads,
        Path::new("/app/lettuce/downloads/onnxruntime")
    );
}

#[test]
fn ready_runtimes_map_to_every_consumer_link() {
    let library = PathBuf::from("/app/onnxruntime/libonnxruntime.so");
    let ready = OnnxRuntimeReady::Library(ResolvedOnnxRuntime {
        library: library.clone(),
        source: OnnxRuntimeSource::Downloaded,
    });
    assert_eq!(
        ready.embeddings_link(),
        OnnxRuntimeLink::Dynamic(library.clone())
    );
    assert_eq!(
        OnnxRuntimeReady::Linked.embeddings_link(),
        OnnxRuntimeLink::Linked
    );
}

#[test]
fn archive_downloads_are_size_checked_below_the_download_folder() {
    let app = TempApp::new();
    let archive = onnx_runtime_archives(OnnxRuntimeHostOs::Linux, "x86_64")
        .expect("linux")
        .remove(0);
    let plan = archive_plan(&app.paths(), &archive, 42);
    assert_eq!(plan.root, app.0.join("downloads/onnxruntime"));
    assert_eq!(
        plan.install_id,
        "onnxruntime:1.22.0:onnxruntime-linux-x64-1.22.0.tgz"
    );
    let planned = &plan.artifacts[0];
    assert_eq!(
        planned.source,
        ArtifactSource::Https {
            url: archive.url.clone()
        }
    );
    assert_eq!(planned.artifact.byte_size, 42);
    assert_eq!(planned.artifact.sha256, None);
    assert_eq!(
        planned.artifact.local_segments,
        ["1.22.0", "onnxruntime-linux-x64-1.22.0.tgz"]
    );
}

/// Runtime evidence is created without a committed environment, which is
/// sound here because no Kokoro session is built.
#[tokio::test]
async fn kokoro_commits_the_runtime_only_when_nothing_committed_it() {
    let calls = std::cell::Cell::new(0);
    let evidence = unsafe { OnnxRuntimeCommitted::after_process_commit() };
    let initialize = |result: Result<OnnxRuntimeCommitted, lettuce_speech::TtsRuntimeError>| {
        let calls = &calls;
        move || {
            calls.set(calls.get() + 1);
            async move { result }
        }
    };

    assert_eq!(
        committed_or_initialize(
            Some(evidence),
            initialize(Err(lettuce_speech::TtsRuntimeError::Unavailable))
        )
        .await,
        Ok(evidence)
    );
    assert_eq!(calls.get(), 0);

    assert_eq!(
        committed_or_initialize(None, initialize(Ok(evidence))).await,
        Ok(evidence)
    );
    assert_eq!(calls.get(), 1);

    assert_eq!(
        committed_or_initialize(
            None,
            initialize(Err(lettuce_speech::TtsRuntimeError::Unavailable))
        )
        .await,
        Err(lettuce_speech::TtsRuntimeError::Unavailable)
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn a_committed_runtime_is_returned_without_touching_the_disk() {
    let paths = OnnxRuntimePaths::legacy_layout(Path::new("/definitely/missing/app"), None);
    let library = PathBuf::from("/definitely/missing/libonnxruntime.so");
    assert_eq!(
        resolve_ready(
            &paths,
            Some(CommittedOnnxRuntime::Library(library.clone())),
            Some(OsStr::new("/also/missing.so")),
        ),
        Some(OnnxRuntimeReady::Library(ResolvedOnnxRuntime {
            library,
            source: OnnxRuntimeSource::Loaded,
        }))
    );
    assert_eq!(
        resolve_ready(&paths, Some(CommittedOnnxRuntime::Linked), None),
        Some(OnnxRuntimeReady::Linked)
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use std::collections::HashMap;
    use std::io::Write;

    use lettuce_jobs::JobState;

    use super::*;

    const LINUX_URL: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/onnxruntime-linux-x64-1.22.0.tgz";

    fn linux_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        for (name, bytes) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, *bytes)
                .expect("entry");
        }
        let mut encoder = builder.into_inner().expect("tar");
        encoder.flush().expect("flush");
        encoder.finish().expect("gzip")
    }

    fn good_archive() -> Vec<u8> {
        linux_archive(&[(
            "onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so.1.22.0",
            b"fake onnxruntime",
        )])
    }

    fn corrupt_archive() -> Vec<u8> {
        linux_archive(&[("onnxruntime-linux-x64-1.22.0/lib/other.so", b"x")])
    }

    struct Body {
        chunks: Vec<Vec<u8>>,
        on_chunk: Option<CancellationToken>,
    }

    #[async_trait]
    impl ArtifactBody for Body {
        fn start(&self) -> u64 {
            0
        }

        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ArtifactSourceError> {
            if let Some(token) = &self.on_chunk {
                token.cancel();
            }
            Ok((!self.chunks.is_empty()).then(|| self.chunks.remove(0)))
        }
    }

    struct Source {
        files: HashMap<String, Vec<u8>>,
        cancel_on_chunk: Option<CancellationToken>,
        opens: Mutex<Vec<String>>,
    }

    impl Source {
        fn serving(files: &[(&str, Vec<u8>)]) -> Self {
            Self {
                files: files
                    .iter()
                    .map(|(url, bytes)| ((*url).to_owned(), bytes.clone()))
                    .collect(),
                cancel_on_chunk: None,
                opens: Mutex::new(Vec::new()),
            }
        }

        fn new(cancel_on_chunk: Option<CancellationToken>) -> Self {
            Self {
                cancel_on_chunk,
                ..Self::serving(&[(LINUX_URL, good_archive())])
            }
        }

        fn opens(&self) -> Vec<String> {
            self.opens.lock().expect("opens").clone()
        }
    }

    #[async_trait]
    impl ArtifactSourceClient for Source {
        async fn open(
            &self,
            source: &ArtifactSource,
            _offset: u64,
            _expected_size: u64,
        ) -> Result<Box<dyn ArtifactBody>, ArtifactSourceError> {
            self.opens.lock().expect("opens").push(source.identity());
            let bytes = self
                .files
                .get(&source.identity())
                .ok_or(ArtifactSourceError::InvalidRequest)?;
            let half = bytes.len() / 2;
            Ok(Box::new(Body {
                chunks: vec![bytes[..half].to_vec(), bytes[half..].to_vec()],
                on_chunk: self.cancel_on_chunk.clone(),
            }))
        }
    }

    #[async_trait]
    impl OnnxRuntimeDownloadSource for Source {
        async fn archive_size(&self, url: &str) -> Result<u64, ArtifactSourceError> {
            self.files
                .get(url)
                .map(|bytes| bytes.len() as u64)
                .ok_or(ArtifactSourceError::InvalidResponse)
        }
    }

    fn archive_named(file_name: &str) -> OnnxRuntimeArchive {
        let mut archive = onnx_runtime_archives(OnnxRuntimeHostOs::Linux, "x86_64")
            .expect("linux")
            .remove(0);
        archive.url = format!("https://example.invalid/{file_name}");
        archive.file_name = file_name.to_owned();
        archive
    }

    fn job_ids(events: &Mutex<Vec<OnnxRuntimeInstallEvent>>) -> Vec<JobId> {
        events
            .lock()
            .expect("events")
            .iter()
            .filter_map(|event| match event {
                OnnxRuntimeInstallEvent::Downloading { job_id, .. } => Some(*job_id),
                OnnxRuntimeInstallEvent::Unpacking { .. } => None,
            })
            .collect()
    }

    fn job_state(database: &Database, id: JobId) -> JobState {
        database.get(id).expect("job").expect("present").state
    }

    #[tokio::test]
    async fn downloads_once_then_reuses_the_unpacked_runtime() {
        let app = TempApp::new();
        let database = Database::open_in_memory().expect("database");
        let installer = OnnxRuntimeInstaller::new(&database, app.paths());
        let source = Source::new(None);
        let events = Mutex::new(Vec::new());
        let record = |event| events.lock().expect("events").push(event);

        assert_eq!(installer.installed_with_override(None), None);
        let ready = installer
            .ensure_with_override(None, &source, &CancellationToken::new(), &record)
            .await
            .expect("installed");
        let library = app.0.join("onnxruntime/libonnxruntime.so");
        assert_eq!(
            ready,
            OnnxRuntimeReady::Library(ResolvedOnnxRuntime {
                library: library.clone(),
                source: OnnxRuntimeSource::Downloaded,
            })
        );
        assert_eq!(
            std::fs::read(&library).expect("library"),
            b"fake onnxruntime"
        );
        assert_eq!(source.opens(), [LINUX_URL]);
        assert!(matches!(
            events.lock().expect("events").as_slice(),
            [
                OnnxRuntimeInstallEvent::Downloading { .. },
                OnnxRuntimeInstallEvent::Unpacking { .. }
            ]
        ));
        assert_eq!(
            job_state(&database, job_ids(&events)[0]),
            JobState::Succeeded
        );
        assert!(
            !app.0
                .join("downloads/onnxruntime/1.22.0/onnxruntime-linux-x64-1.22.0.tgz")
                .exists(),
            "the archive is deleted once unpacked"
        );

        let again = installer
            .ensure_with_override(None, &source, &CancellationToken::new(), &no_events)
            .await
            .expect("reused");
        assert_eq!(again, ready);
        assert_eq!(source.opens().len(), 1);
    }

    #[tokio::test]
    async fn an_unpack_failure_fails_the_job_and_deletes_the_archive() {
        let app = TempApp::new();
        let database = Database::open_in_memory().expect("database");
        let installer = OnnxRuntimeInstaller::new(&database, app.paths());
        let source = Source::serving(&[(LINUX_URL, corrupt_archive())]);
        let events = Mutex::new(Vec::new());
        let record = |event| events.lock().expect("events").push(event);
        let result = installer
            .ensure_with_override(None, &source, &CancellationToken::new(), &record)
            .await;
        assert!(matches!(
            result,
            Err(OnnxRuntimeInstallError::Unpack(
                OnnxRuntimeProvisionError::MissingEntry(_)
            ))
        ));
        assert_eq!(job_state(&database, job_ids(&events)[0]), JobState::Failed);
        assert!(
            !app.0
                .join("downloads/onnxruntime/1.22.0/onnxruntime-linux-x64-1.22.0.tgz")
                .exists()
        );
        assert!(!app.0.join("onnxruntime/libonnxruntime.so").exists());

        let result = installer
            .ensure_with_override(None, &source, &CancellationToken::new(), &record)
            .await;
        assert!(result.is_err());
        assert_eq!(
            source.opens().len(),
            2,
            "the rejected archive is downloaded afresh"
        );
    }

    #[tokio::test]
    async fn a_rejected_archive_falls_back_to_the_next_one() {
        let app = TempApp::new();
        let database = Database::open_in_memory().expect("database");
        let installer = OnnxRuntimeInstaller::new(&database, app.paths());
        let arch = archive_named("onnxruntime-arch-1.22.0.tgz");
        let universal = archive_named("onnxruntime-universal-1.22.0.tgz");
        let source = Source::serving(&[
            (&arch.url, corrupt_archive()),
            (&universal.url, good_archive()),
        ]);
        let resolved = installer
            .install_first(
                OnnxRuntimeHostOs::Linux,
                vec![arch.clone(), universal.clone()],
                &source,
                &CancellationToken::new(),
                &no_events,
            )
            .await
            .expect("fallback");
        assert_eq!(
            std::fs::read(resolved.library).expect("library"),
            b"fake onnxruntime"
        );
        assert_eq!(source.opens(), [arch.url, universal.url]);
        assert!(
            !app.0
                .join("downloads/onnxruntime/1.22.0/onnxruntime-arch-1.22.0.tgz")
                .exists()
        );
    }

    #[tokio::test]
    async fn a_download_claimed_elsewhere_is_busy() {
        let app = TempApp::new();
        let database = Database::open_in_memory().expect("database");
        let installer = OnnxRuntimeInstaller::new(&database, app.paths());
        let source = Source::new(None);
        let archive = onnx_runtime_archives(OnnxRuntimeHostOs::Linux, "x86_64")
            .expect("linux")
            .remove(0);
        let plan = archive_plan(&app.paths(), &archive, source.files[LINUX_URL].len() as u64);
        let coordinator = ArtifactInstallCoordinator::new(&database);
        let admitted = coordinator.admit(&plan).expect("admit");
        let _held = coordinator
            .claim(
                plan,
                admitted.job.id,
                WorkerId::new(),
                TimestampMillis::now().expect("now"),
                Duration::from_secs(600),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("claimed");
        let result = installer
            .ensure_with_override(None, &source, &CancellationToken::new(), &no_events)
            .await;
        assert!(matches!(result, Err(OnnxRuntimeInstallError::Busy)));
        assert!(source.opens().is_empty());
    }

    #[tokio::test]
    async fn concurrent_ensures_share_one_download() {
        let app = TempApp::new();
        let database = Database::open_in_memory().expect("database");
        let installer = OnnxRuntimeInstaller::new(&database, app.paths());
        let source = Source::new(None);
        let token = CancellationToken::new();
        let (first, second) = tokio::join!(
            installer.ensure_with_override(None, &source, &token, &no_events),
            installer.ensure_with_override(None, &source, &token, &no_events),
        );
        assert_eq!(first.expect("first"), second.expect("second"));
        assert_eq!(source.opens().len(), 1);
    }

    #[tokio::test]
    async fn a_zero_byte_runtime_heals_by_downloading_again() {
        let app = TempApp::new();
        let library = app.0.join("onnxruntime/libonnxruntime.so");
        std::fs::create_dir_all(library.parent().expect("parent")).expect("dir");
        std::fs::write(&library, b"").expect("empty");
        let database = Database::open_in_memory().expect("database");
        let installer = OnnxRuntimeInstaller::new(&database, app.paths());
        let source = Source::new(None);
        installer
            .ensure_with_override(None, &source, &CancellationToken::new(), &no_events)
            .await
            .expect("healed");
        assert_eq!(
            std::fs::read(&library).expect("library"),
            b"fake onnxruntime"
        );
        assert_eq!(source.opens().len(), 1);
    }

    #[tokio::test]
    async fn an_override_skips_the_download() {
        let app = TempApp::new();
        let custom = app.0.join("custom.so");
        std::fs::write(&custom, b"custom").expect("custom");
        let database = Database::open_in_memory().expect("database");
        let installer = OnnxRuntimeInstaller::new(&database, app.paths());
        let source = Source::new(None);
        let ready = installer
            .ensure_with_override(
                Some(custom.clone().into_os_string()),
                &source,
                &CancellationToken::new(),
                &no_events,
            )
            .await
            .expect("override");
        assert_eq!(
            ready,
            OnnxRuntimeReady::Library(ResolvedOnnxRuntime {
                library: custom,
                source: OnnxRuntimeSource::Override,
            })
        );
        assert!(source.opens().is_empty());
    }

    #[tokio::test]
    async fn cancelling_stops_the_download_without_a_runtime() {
        let app = TempApp::new();
        let database = Database::open_in_memory().expect("database");
        let installer = OnnxRuntimeInstaller::new(&database, app.paths());
        let cancellation = CancellationToken::new();
        let source = Source::new(Some(cancellation.clone()));
        let result = installer
            .ensure_with_override(None, &source, &cancellation, &no_events)
            .await;
        assert!(matches!(result, Err(OnnxRuntimeInstallError::Cancelled)));
        assert!(!app.0.join("onnxruntime/libonnxruntime.so").exists());

        let result = installer
            .ensure_with_override(None, &source, &cancellation, &no_events)
            .await;
        assert!(matches!(result, Err(OnnxRuntimeInstallError::Cancelled)));
        assert_eq!(source.opens().len(), 1, "a cancelled token starts nothing");
    }

    struct FailingSucceed(Database);

    impl lettuce_jobs::JobStore for FailingSucceed {
        fn create_or_get(
            &self,
            spec: lettuce_jobs::NewJob,
        ) -> Result<lettuce_jobs::CreateJobResult, lettuce_jobs::StoreError> {
            self.0.create_or_get(spec)
        }

        fn get(
            &self,
            id: JobId,
        ) -> Result<Option<lettuce_jobs::JobSnapshot>, lettuce_jobs::StoreError> {
            self.0.get(id)
        }

        fn list(
            &self,
            query: lettuce_jobs::JobQuery,
        ) -> Result<lettuce_types::Page<lettuce_jobs::JobSnapshot>, lettuce_jobs::StoreError>
        {
            self.0.list(query)
        }

        fn events_since(
            &self,
            id: JobId,
            after: Option<lettuce_jobs::EventSeq>,
            limit: u32,
        ) -> Result<Vec<lettuce_jobs::events::JobEventEnvelope>, lettuce_jobs::StoreError> {
            self.0.events_since(id, after, limit)
        }

        fn claim_next(
            &self,
            worker_id: WorkerId,
            now: lettuce_jobs::Timestamp,
            lease_for: Duration,
            allowed: &ResourceAvailability,
        ) -> Result<Option<lettuce_jobs::Claim>, lettuce_jobs::StoreError> {
            self.0.claim_next(worker_id, now, lease_for, allowed)
        }

        fn claim(
            &self,
            id: JobId,
            worker_id: WorkerId,
            now: lettuce_jobs::Timestamp,
            lease_for: Duration,
            allowed: &ResourceAvailability,
        ) -> Result<Option<lettuce_jobs::Claim>, lettuce_jobs::StoreError> {
            self.0.claim(id, worker_id, now, lease_for, allowed)
        }

        fn heartbeat(
            &self,
            claim: &lettuce_jobs::ClaimRef,
            now: lettuce_jobs::Timestamp,
            extend_for: Duration,
        ) -> Result<lettuce_jobs::Claim, lettuce_jobs::StoreError> {
            self.0.heartbeat(claim, now, extend_for)
        }

        fn append_and_transition(
            &self,
            mutation: lettuce_jobs::JobMutation,
        ) -> Result<lettuce_jobs::JobSnapshot, lettuce_jobs::StoreError> {
            if matches!(mutation, lettuce_jobs::JobMutation::Succeed { .. }) {
                return Err(lettuce_jobs::StoreError::Storage);
            }
            self.0.append_and_transition(mutation)
        }

        fn expired_claims(
            &self,
            now: lettuce_jobs::Timestamp,
            limit: u32,
        ) -> Result<Vec<lettuce_jobs::ExpiredClaim>, lettuce_jobs::StoreError> {
            self.0.expired_claims(now, limit)
        }

        fn orphaned_claims(
            &self,
            now: lettuce_jobs::Timestamp,
            limit: u32,
        ) -> Result<Vec<lettuce_jobs::ExpiredClaim>, lettuce_jobs::StoreError> {
            self.0.orphaned_claims(now, limit)
        }

        fn prune(
            &self,
            policy: lettuce_jobs::retention::RetentionPolicy,
            now: lettuce_jobs::Timestamp,
        ) -> Result<lettuce_jobs::PruneReport, lettuce_jobs::StoreError> {
            self.0.prune(policy, now)
        }
    }

    #[tokio::test]
    async fn an_installed_runtime_survives_a_failed_job_record() {
        let app = TempApp::new();
        let jobs = FailingSucceed(Database::open_in_memory().expect("database"));
        let installer = OnnxRuntimeInstaller::new(&jobs, app.paths());
        let source = Source::new(None);
        let events = Mutex::new(Vec::new());
        let record = |event| events.lock().expect("events").push(event);
        let ready = installer
            .ensure_with_override(None, &source, &CancellationToken::new(), &record)
            .await
            .expect("the unpacked runtime is returned");
        assert_eq!(
            job_state(&jobs.0, job_ids(&events)[0]),
            JobState::Failed,
            "the job is closed instead of staying running"
        );
        let library = app.0.join("onnxruntime/libonnxruntime.so");
        assert_eq!(
            ready,
            OnnxRuntimeReady::Library(ResolvedOnnxRuntime {
                library: library.clone(),
                source: OnnxRuntimeSource::Downloaded,
            })
        );
        assert_eq!(
            std::fs::read(library).expect("library"),
            b"fake onnxruntime"
        );
    }

    struct Unreachable;

    #[async_trait]
    impl ArtifactSourceClient for Unreachable {
        async fn open(
            &self,
            _source: &ArtifactSource,
            _offset: u64,
            _expected_size: u64,
        ) -> Result<Box<dyn ArtifactBody>, ArtifactSourceError> {
            panic!("a committed runtime must not be downloaded again");
        }
    }

    #[async_trait]
    impl OnnxRuntimeDownloadSource for Unreachable {
        async fn archive_size(&self, _url: &str) -> Result<u64, ArtifactSourceError> {
            panic!("a committed runtime must not be looked up again");
        }
    }

    #[tokio::test]
    #[ignore = "downloads ONNX Runtime from GitHub and loads it into this process"]
    async fn live_downloads_and_initializes_the_linux_runtime() {
        let app = TempApp::new();
        let database = Database::open_in_memory().expect("database");
        let installer = OnnxRuntimeInstaller::new(&database, app.paths());
        let client = ArtifactDownloadClient::new().expect("client");
        let ready = installer
            .ensure_with_override(None, &client, &CancellationToken::new(), &no_events)
            .await
            .expect("installed");
        let OnnxRuntimeReady::Library(resolved) = &ready else {
            panic!("expected a downloaded library");
        };
        assert_eq!(resolved.source, OnnxRuntimeSource::Downloaded);
        assert!(std::fs::metadata(&resolved.library).expect("library").len() > 1_000_000);
        assert_eq!(committed_kokoro_runtime(), None);
        assert!(ready.initialize().is_ok());
        assert!(ready.initialize().is_ok());
        assert!(committed_kokoro_runtime().is_some());

        std::fs::remove_dir_all(app.0.join("onnxruntime")).expect("remove install");
        let committed = installer
            .ensure_with_override(None, &Unreachable, &CancellationToken::new(), &no_events)
            .await
            .expect("committed");
        assert_eq!(
            committed,
            OnnxRuntimeReady::Library(ResolvedOnnxRuntime {
                library: resolved.library.clone(),
                source: OnnxRuntimeSource::Loaded,
            })
        );
        assert_eq!(installer.installed(), Some(committed));
    }
}
