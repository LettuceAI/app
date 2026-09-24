use std::path::Path;

use lettuce_model_hub::{
    InstalledWhisperManifest, VerifiedWhisperArtifacts, WhisperInstallStore, WhisperModelError,
    WhisperModelRepository, WhisperModelRepositoryError, inspect_legacy_whisper_models,
    select_default_whisper_model,
};
use lettuce_speech::{AsrModelDescriptor, AsrModelId, AsrRuntimeError, WhisperCppRuntime};
use lettuce_types::TimestampMillis;

#[derive(Debug, thiserror::Error)]
pub enum WhisperModelCoordinatorError {
    #[error("Whisper model inspection failed: {0}")]
    Model(#[from] WhisperModelError),
    #[error("Whisper model persistence failed: {0}")]
    Repository(#[from] WhisperModelRepositoryError),
    #[error("Whisper runtime failed: {0}")]
    Runtime(#[from] AsrRuntimeError),
    #[error("Whisper model id is invalid")]
    InvalidModelId,
    #[error("Whisper model is not installed")]
    NotInstalled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WhisperModelRemoval {
    pub removed: bool,
    pub file_removed: bool,
    pub cleared_cached_contexts: usize,
}

#[derive(Debug)]
pub struct WhisperModelCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: ?Sized> WhisperModelCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }
}

impl<R: WhisperModelRepository + ?Sized> WhisperModelCoordinator<'_, R> {
    pub fn admit_retained_legacy_directory(
        &self,
        legacy_models_root: &Path,
        admitted_at: TimestampMillis,
    ) -> Result<Vec<InstalledWhisperManifest>, WhisperModelCoordinatorError> {
        inspect_legacy_whisper_models(legacy_models_root, admitted_at)?
            .into_iter()
            .map(|manifest| {
                self.repository
                    .admit_whisper_model(manifest)
                    .map_err(Into::into)
            })
            .collect()
    }

    pub fn list(&self) -> Result<Vec<InstalledWhisperManifest>, WhisperModelCoordinatorError> {
        self.repository.list_whisper_models().map_err(Into::into)
    }

    pub fn resolve(
        &self,
        requested_model_id: Option<&str>,
    ) -> Result<(AsrModelDescriptor, VerifiedWhisperArtifacts), WhisperModelCoordinatorError> {
        let manifest = match requested_model_id {
            Some(model_id) => self
                .repository
                .get_whisper_model(model_id)?
                .ok_or(WhisperModelCoordinatorError::NotInstalled)?,
            None => select_default_whisper_model(&self.repository.list_whisper_models()?)?
                .ok_or(WhisperModelCoordinatorError::NotInstalled)?,
        };
        let verified = manifest.verify()?;
        let descriptor = AsrModelDescriptor {
            id: AsrModelId::new(verified.model_id.clone())
                .map_err(|_| WhisperModelCoordinatorError::InvalidModelId)?,
            artifact_hash: verified.blake3.clone(),
            english_only: verified.english_only,
        };
        Ok((descriptor, verified))
    }

    pub fn remove_managed(
        &self,
        installs: &WhisperInstallStore,
        runtime: &WhisperCppRuntime<R>,
        model_id: &str,
    ) -> Result<WhisperModelRemoval, WhisperModelCoordinatorError> {
        let Some(manifest) = self.repository.get_whisper_model(model_id)? else {
            return Ok(WhisperModelRemoval {
                removed: false,
                file_removed: false,
                cleared_cached_contexts: 0,
            });
        };
        installs.validate_managed(&manifest)?;
        let cleared_cached_contexts = runtime.clear_cache()?;
        let file_removed = installs.remove_managed(&manifest)?;
        let removed = self.repository.remove_whisper_model(&manifest)?;
        Ok(WhisperModelRemoval {
            removed,
            file_removed,
            cleared_cached_contexts,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lettuce_database::Database;
    use lettuce_model_hub::{
        RemoteWhisperModel, WhisperInstallPreparation, WhisperInstallStore, WhisperModelRepository,
    };
    use lettuce_types::{OperationId, TimestampMillis};

    use super::*;

    #[test]
    fn admits_retained_models_selects_legacy_first_and_replays_after_reopen() {
        let root = std::env::temp_dir().join(format!("legacy-whisper-{}", OperationId::new()));
        for (id, bytes) in [
            ("small.en", b"small".as_slice()),
            ("base", b"base".as_slice()),
        ] {
            let folder = root.join(id);
            std::fs::create_dir_all(&folder).expect("model directory");
            std::fs::write(folder.join(format!("ggml-{id}.bin")), bytes).expect("model");
        }
        let source_before = std::fs::read(root.join("base/ggml-base.bin")).expect("source bytes");
        let database_path = root.join("catalog.sqlite3");
        let selected_id;
        {
            let database = Database::open(&database_path).expect("database");
            let coordinator = WhisperModelCoordinator::new(&database);
            let admitted = coordinator
                .admit_retained_legacy_directory(&root, TimestampMillis::new(10))
                .expect("admitted models");
            assert_eq!(admitted.len(), 2);
            let (descriptor, verified) = coordinator.resolve(None).expect("default model");
            assert_eq!(descriptor.id.as_str(), "base");
            assert_eq!(verified.model_id, "base");
            selected_id = descriptor.id.as_str().to_owned();
        }
        let database = Database::open(&database_path).expect("reopen database");
        let coordinator = WhisperModelCoordinator::new(&database);
        let repeated = coordinator
            .admit_retained_legacy_directory(&root, TimestampMillis::new(10))
            .expect("replayed admission");
        assert_eq!(repeated.len(), 2);
        assert_eq!(coordinator.list().expect("list models").len(), 2);
        assert_eq!(
            coordinator.resolve(None).expect("resolve").0.id.as_str(),
            selected_id
        );
        assert_eq!(
            std::fs::read(root.join("base/ggml-base.bin")).expect("source bytes after"),
            source_before
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn changed_source_cannot_replace_an_admitted_manifest() {
        let root = std::env::temp_dir().join(format!("legacy-whisper-{}", OperationId::new()));
        let folder = root.join("base");
        std::fs::create_dir_all(&folder).expect("model directory");
        let path = folder.join("ggml-base.bin");
        std::fs::write(&path, b"original").expect("model");
        let database = Database::open_in_memory().expect("database");
        let coordinator = WhisperModelCoordinator::new(&database);
        coordinator
            .admit_retained_legacy_directory(&root, TimestampMillis::new(10))
            .expect("admitted model");
        std::fs::write(&path, b"changed").expect("changed model");
        assert!(matches!(
            coordinator.resolve(Some("base")),
            Err(WhisperModelCoordinatorError::Model(
                WhisperModelError::Mismatch
            ))
        ));
        let replacement =
            InstalledWhisperManifest::inspect_legacy(&root, &path, TimestampMillis::new(11))
                .expect("replacement manifest");
        assert_eq!(
            WhisperModelRepository::admit_whisper_model(&database, replacement),
            Err(WhisperModelRepositoryError::Conflict)
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn removes_only_managed_models_after_clearing_runtime_cache_and_replays() {
        let root = std::env::temp_dir().join(format!("managed-whisper-{}", OperationId::new()));
        let install_root = root.join("models");
        let store = WhisperInstallStore::open(&install_root).expect("install store");
        let remote = RemoteWhisperModel::pinned(
            "ggml-base.bin",
            "ab".repeat(20),
            13,
            "012645b3bdfb7d07c105768b7c2bed0352e09c52b6064747b348c0a4866daee1",
        )
        .expect("remote model");
        let WhisperInstallPreparation::Download(mut download) = store
            .prepare(remote, TimestampMillis::new(10))
            .expect("download")
        else {
            panic!("expected download");
        };
        download.append(b"managed model").expect("write model");
        let manifest = download.finish().expect("finish model");
        let database = Arc::new(Database::open_in_memory().expect("database"));
        WhisperModelRepository::admit_whisper_model(database.as_ref(), manifest.clone())
            .expect("admit manifest");
        let runtime = WhisperCppRuntime::new(database.clone());
        let coordinator = WhisperModelCoordinator::new(database.as_ref());
        let removed = coordinator
            .remove_managed(&store, &runtime, "base")
            .expect("remove model");
        assert!(removed.removed);
        assert!(removed.file_removed);
        assert_eq!(removed.cleared_cached_contexts, 0);
        assert!(!manifest.model.path.exists());
        assert!(
            WhisperModelRepository::get_whisper_model(database.as_ref(), "base")
                .expect("model lookup")
                .is_none()
        );
        assert_eq!(
            coordinator
                .remove_managed(&store, &runtime, "base")
                .expect("removal replay"),
            WhisperModelRemoval {
                removed: false,
                file_removed: false,
                cleared_cached_contexts: 0,
            }
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn refuses_to_remove_a_retained_legacy_model() {
        let root = std::env::temp_dir().join(format!("legacy-whisper-{}", OperationId::new()));
        let legacy_root = root.join("legacy");
        let folder = legacy_root.join("base");
        std::fs::create_dir_all(&folder).expect("legacy model directory");
        let path = folder.join("ggml-base.bin");
        std::fs::write(&path, b"legacy").expect("legacy model");
        let manifest =
            InstalledWhisperManifest::inspect_legacy(&legacy_root, &path, TimestampMillis::new(10))
                .expect("legacy manifest");
        let database = Arc::new(Database::open_in_memory().expect("database"));
        WhisperModelRepository::admit_whisper_model(database.as_ref(), manifest)
            .expect("admit legacy model");
        let runtime = WhisperCppRuntime::new(database.clone());
        let coordinator = WhisperModelCoordinator::new(database.as_ref());
        let store = WhisperInstallStore::open(root.join("managed")).expect("install store");
        assert!(matches!(
            coordinator.remove_managed(&store, &runtime, "base"),
            Err(WhisperModelCoordinatorError::Model(
                WhisperModelError::OutsideSource
            ))
        ));
        assert!(path.exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
