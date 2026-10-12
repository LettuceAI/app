use lettuce_characters::{
    Crop, LifecycleStatus, Persona, PersonaMedia, PersonaRepository, RepositoryError,
};
#[cfg(test)]
use lettuce_characters::{PersonaMediaLink, PersonaMediaSlot};
use lettuce_context::{
    BindingRepositoryError, LifecycleStatus as LorebookStatus, LorebookRepository,
    PersonaLorebookBindingRepository,
};
#[cfg(test)]
use lettuce_media::AssetKind;
use lettuce_media::{LocalMediaBlobStore, MediaAssetRepository, MediaBlobRepository};
use lettuce_transfer::{
    EntityPackageError, LegacyImportSkip, LegacyImportSkipKind, LegacyImportSkipReason,
    PersonaFileImport, PersonaFileRepository, PersonaPackage,
};
use lettuce_types::{LorebookId, PersonaId, Revision, TimestampMillis};

#[cfg(test)]
use crate::files::character_files::store_file_image;

#[derive(Debug, thiserror::Error)]
pub enum PersonaFileError {
    #[error(transparent)]
    Package(#[from] EntityPackageError),
    #[error("persona storage failed: {0}")]
    Repository(#[from] RepositoryError),
    #[error("persona lorebook storage failed: {0}")]
    Bindings(#[from] BindingRepositoryError),
    #[error("Persona not found")]
    NotFound,
}

/// A persona file written as a new persona.
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct ImportedPersonaFile {
    pub persona: Persona,
    pub skipped: Vec<LegacyImportSkip>,
}

/// Compatibility persona packages read into new personas.
#[derive(Debug)]
pub struct PersonaFileCoordinator<'a, R: ?Sized, BR, AR> {
    repository: &'a R,
    _media_store: &'a LocalMediaBlobStore<BR, AR>,
}

impl<'a, R: ?Sized, BR, AR> PersonaFileCoordinator<'a, R, BR, AR> {
    #[must_use]
    pub const fn new(repository: &'a R, media_store: &'a LocalMediaBlobStore<BR, AR>) -> Self {
        Self {
            repository,
            _media_store: media_store,
        }
    }
}

impl<R, BR, AR> PersonaFileCoordinator<'_, R, BR, AR>
where
    R: PersonaRepository
        + PersonaFileRepository
        + PersonaLorebookBindingRepository
        + LorebookRepository
        + ?Sized,
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    /// The package in a persona UEC or the pre-UEC persona package; the host
    /// may replace a remote `avatar_data` with the downloaded image's data URL
    /// before importing it.
    pub fn read(
        &self,
        json: &str,
        now: TimestampMillis,
    ) -> Result<PersonaPackage, PersonaFileError> {
        let value = serde_json::from_str(json)
            .map_err(|error| EntityPackageError::InvalidImport(error.to_string()))?;
        Ok(lettuce_transfer::parse_persona_import(&value, now.get())?)
    }

    /// Writes the package as a new persona bound to the file's lorebooks that
    /// exist here, made the default when the file says so; an avatar that
    /// cannot be stored is left off.
    #[cfg(test)]
    pub fn import(
        &self,
        package: &PersonaPackage,
        now: TimestampMillis,
    ) -> Result<ImportedPersonaFile, PersonaFileError> {
        let (mut import, skipped) = self.prepare(package, now)?;
        if let Some(asset_id) = package.avatar_data.as_deref().and_then(|avatar| {
            store_file_image(self._media_store, avatar, AssetKind::AvatarOriginal, "lettuceai-entity-file")
        }) {
            import.persona.media.links.push(PersonaMediaLink { asset_id, slot: PersonaMediaSlot::Avatar, ordinal: 0 });
        }
        let avatar = import.persona.media.links.first().map(|link| link.asset_id);
        match self.repository.import_persona_file(&import) {
            Ok(persona) => Ok(ImportedPersonaFile {
                persona,
                skipped,
            }),
            Err(error) => {
                if let Some(asset_id) = avatar {
                    self.discard_avatar(asset_id, now);
                }
                Err(error.into())
            }
        }
    }

    pub fn prepare(
        &self,
        package: &PersonaPackage,
        now: TimestampMillis,
    ) -> Result<(PersonaFileImport, Vec<LegacyImportSkip>), PersonaFileError> {
        let data = &package.persona;
        let persona = Persona {
            id: PersonaId::new(),
            status: LifecycleStatus::Active,
            title: data.title.clone(),
            description: data.description.clone(),
            nickname: data.nickname.clone(),
            design_description: None,
            avatar_crop: data
                .avatar_crop
                .as_ref()
                .and_then(|crop| Crop::new(crop.x as f32, crop.y as f32, crop.scale as f32).ok()),
            image_recommendation: None,
            media: PersonaMedia::default(),
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        persona.validate().map_err(RepositoryError::from)?;
        let mut skipped = Vec::new();
        let mut bound = Vec::new();
        for source_id in &data.active_lorebook_ids {
            let lorebook_id = source_id.parse::<LorebookId>().ok();
            if lorebook_id.is_some_and(|id| bound.contains(&id)) {
                continue;
            }
            let active = lorebook_id
                .map(|id| LorebookRepository::get(self.repository, id))
                .transpose()
                .map_err(|error| {
                    PersonaFileError::Bindings(BindingRepositoryError::Failure(error.to_string()))
                })?
                .flatten()
                .filter(|details| details.book.status == LorebookStatus::Active);
            let Some(lorebook) = active else {
                skipped.push(LegacyImportSkip {
                    kind: LegacyImportSkipKind::LorebookReference,
                    source_key: format!("personas.active_lorebook_ids:{source_id}"),
                    reason: LegacyImportSkipReason::MissingLorebook,
                });
                continue;
            };
            bound.push(lorebook.book.id);
        }
        Ok((
            PersonaFileImport {
                persona,
                lorebook_ids: bound,
                disabled_lorebook_ids: Vec::new(),
                make_default: data.is_default == Some(true),
            },
            skipped,
        ))
    }

    /// Deletes an avatar stored for an import that failed, so it leaves no
    /// unused file behind; a failure here is only logged.
    #[cfg(test)]
    fn discard_avatar(&self, asset_id: lettuce_types::AssetId, now: TimestampMillis) {
        let blob_id = match self.repository.discard_unlinked_asset(asset_id) {
            Ok(Some(blob_id)) => blob_id,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(%error, "unused persona avatar was not deleted");
                return;
            }
        };
        if let Err(error) = self._media_store.release_blob(blob_id, |_| {
            self.repository
                .release_unused_blob(blob_id, now)
                .map_err(|_| lettuce_media::MediaStoreError::CatalogFailure)
        }) {
            tracing::warn!(%error, "unused persona avatar bytes were not deleted");
        }
    }


}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use lettuce_context::{DetectionPolicy, LorebookBehaviorVersion, LorebookMetadataDraft};
    use lettuce_database::Database;
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_types::OperationId;

    use super::*;

    fn png() -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&2_u32.to_be_bytes());
        bytes.extend_from_slice(&3_u32.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(b"persona avatar");
        bytes
    }

    #[test]
    fn a_persona_uec_without_a_description_imports_like_legacy_parse_uec_persona() {
        let root = std::env::temp_dir().join(format!("persona-blank-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let path = root.join("state.sqlite3");
        let backend = crate::AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("authority");
        let store = LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write"),
            Database::open(&path).expect("blobs"),
            Database::open(&path).expect("assets"),
        );
        let mut uec: serde_json::Value = serde_json::from_str(include_str!(
            "../../../lettuce-transfer/tests/fixtures/uec2-persona-blank.json"
        ))
        .expect("fixture");
        uec["payload"]
            .as_object_mut()
            .expect("payload")
            .remove("description");
        let files = backend.persona_files(&store);
        let package = files
            .read(&uec.to_string(), TimestampMillis::new(5))
            .expect("read");
        let imported = files
            .import(&package, TimestampMillis::new(5))
            .expect("a title-only persona imports");
        assert_eq!(imported.persona.title, "Wanderer");
        assert_eq!(imported.persona.description, "");
        assert_eq!(
            PersonaRepository::get_default_snapshot(backend.database())
                .expect("default")
                .state
                .persona_id,
            Some(imported.persona.id)
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_persona_file_round_trips_through_a_new_default_persona() {
        let root = std::env::temp_dir().join(format!("persona-file-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let path = root.join("state.sqlite3");
        let backend = crate::AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("authority");
        let store = LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write"),
            Database::open(&path).expect("blobs"),
            Database::open(&path).expect("assets"),
        );
        let lorebook = LorebookRepository::create(
            backend.database(),
            LorebookMetadataDraft {
                name: "Harbour".to_owned(),
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            Vec::new(),
            TimestampMillis::new(2),
        )
        .expect("lorebook");
        let avatar = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png())
        );
        let files = backend.persona_files(&store);
        let package = files
            .read(
                &serde_json::json!({
                    "version": 1,
                    "exportedAt": 5,
                    "persona": {
                        "title": "Mira",
                        "description": "A traveller",
                        "nickname": "Mi",
                        "isDefault": true,
                        "avatarCrop": {"x": 0.3, "y": 0.1, "scale": 1.5},
                        "activeLorebookIds": [lorebook.book.id.to_string(), "missing"]
                    },
                    "avatarData": avatar
                })
                .to_string(),
                TimestampMillis::new(10),
            )
            .expect("read");
        let imported = files
            .import(&package, TimestampMillis::new(10))
            .expect("import");
        let persona = &imported.persona;
        assert_eq!(persona.title, "Mira");
        assert_eq!(persona.media.links.len(), 1);
        assert_eq!(imported.skipped.len(), 1);
        assert_eq!(
            backend
                .database()
                .list_persona_bindings(persona.id)
                .expect("bindings")
                .len(),
            1
        );
        assert_eq!(
            PersonaRepository::get_default_snapshot(backend.database())
                .expect("default")
                .state
                .persona_id,
            Some(persona.id)
        );
        let again = PersonaRepository::get(backend.database(), persona.id)
            .expect("read")
            .expect("persona");
        assert_eq!(again.title, "Mira");
        assert_eq!(again.nickname.as_deref(), Some("Mi"));
        assert_eq!(again.avatar_crop.map(|crop| crop.x), Some(0.3f32));
        let bindings = backend
            .database()
            .list_persona_bindings(again.id)
            .expect("bindings");
        assert_eq!(
            bindings
                .iter()
                .map(|binding| binding.lorebook_id.to_string())
                .collect::<Vec<_>>(),
            vec![lorebook.book.id.to_string()]
        );
        let mut opened = store
            .open_ready(again.media.links[0].asset_id)
            .expect("avatar");
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut opened.reader, &mut bytes).expect("bytes");
        assert_eq!(bytes, png());
        std::fs::remove_dir_all(&root).ok();
    }
}
