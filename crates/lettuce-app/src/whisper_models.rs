use std::path::Path;

use lettuce_model_hub::{
    InstalledWhisperManifest, VerifiedWhisperArtifacts, WhisperModelError, WhisperModelRepository,
    WhisperModelRepositoryError, inspect_legacy_whisper_models, select_default_whisper_model,
};
use lettuce_speech::{AsrModelDescriptor, AsrModelId};
use lettuce_types::TimestampMillis;

#[derive(Debug, thiserror::Error)]
pub enum WhisperModelCoordinatorError {
    #[error("Whisper model inspection failed: {0}")]
    Model(#[from] WhisperModelError),
    #[error("Whisper model persistence failed: {0}")]
    Repository(#[from] WhisperModelRepositoryError),
    #[error("Whisper model id is invalid")]
    InvalidModelId,
    #[error("Whisper model is not installed")]
    NotInstalled,
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
}

#[cfg(test)]
mod tests {
    use lettuce_database::Database;
    use lettuce_model_hub::WhisperModelRepository;
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
}
