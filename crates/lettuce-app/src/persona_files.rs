use lettuce_characters::{
    Crop, LifecycleStatus, Persona, PersonaMedia, PersonaMediaLink, PersonaMediaSlot,
    PersonaRepository, RepositoryError,
};
use lettuce_context::{
    BindingInsertionTarget, BindingRepositoryError, LifecycleStatus as LorebookStatus,
    LorebookBindingCreate, LorebookRepository, PersonaLorebookBindingRepository,
};
use lettuce_media::{AssetKind, LocalMediaBlobStore, MediaAssetRepository, MediaBlobRepository};
use lettuce_transfer::{
    EntityPackageError, LegacyImportSkip, LegacyImportSkipKind, LegacyImportSkipReason,
    PackageCrop, PersonaPackage, PersonaUecSource,
};
use lettuce_types::{LorebookId, PersonaId, Revision, TimestampMillis};

use crate::{inline_image, store_file_image};

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
#[derive(Debug, Clone)]
pub struct ImportedPersonaFile {
    pub persona: Persona,
    pub skipped: Vec<LegacyImportSkip>,
}

/// Persona files read into new personas and written from stored ones.
#[derive(Debug)]
pub struct PersonaFileCoordinator<'a, R: ?Sized, BR, AR> {
    repository: &'a R,
    media_store: &'a LocalMediaBlobStore<BR, AR>,
}

impl<'a, R: ?Sized, BR, AR> PersonaFileCoordinator<'a, R, BR, AR> {
    #[must_use]
    pub const fn new(repository: &'a R, media_store: &'a LocalMediaBlobStore<BR, AR>) -> Self {
        Self {
            repository,
            media_store,
        }
    }
}

impl<R, BR, AR> PersonaFileCoordinator<'_, R, BR, AR>
where
    R: PersonaRepository + PersonaLorebookBindingRepository + LorebookRepository + ?Sized,
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
    pub fn import(
        &self,
        package: &PersonaPackage,
        now: TimestampMillis,
    ) -> Result<ImportedPersonaFile, PersonaFileError> {
        let data = &package.persona;
        let mut persona = Persona {
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
        if let Some(asset_id) = package.avatar_data.as_deref().and_then(|avatar| {
            store_file_image(
                self.media_store,
                avatar,
                AssetKind::AvatarOriginal,
                "lettuceai-persona-file",
            )
        }) {
            persona.media.links.push(PersonaMediaLink {
                asset_id,
                slot: PersonaMediaSlot::Avatar,
                ordinal: 0,
            });
        }
        let persona = PersonaRepository::create(self.repository, persona)?;
        let mut skipped = Vec::new();
        let mut revision = persona.revision;
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
            revision = self
                .repository
                .bind_persona_lorebook(
                    persona.id,
                    revision,
                    LorebookBindingCreate {
                        lorebook_id: lorebook.book.id,
                        target: BindingInsertionTarget::Append,
                    },
                    now,
                )?
                .owner_revision;
            bound.push(lorebook.book.id);
        }
        if data.is_default == Some(true) {
            let current = self.repository.get_default_snapshot()?.state.revision;
            self.repository.set_default(persona.id, current, now)?;
        }
        let persona = PersonaRepository::get(self.repository, persona.id)?
            .ok_or(PersonaFileError::NotFound)?;
        Ok(ImportedPersonaFile { persona, skipped })
    }

    /// The persona written as a v2 persona UEC with its avatar inlined.
    pub fn export(&self, persona_id: PersonaId) -> Result<String, PersonaFileError> {
        let persona = PersonaRepository::get(self.repository, persona_id)?
            .ok_or(PersonaFileError::NotFound)?;
        let mut bindings = self.repository.list_persona_bindings(persona_id)?;
        bindings.sort_by_key(|binding| binding.ordinal);
        let is_default =
            self.repository.get_default_snapshot()?.state.persona_id == Some(persona_id);
        let decimal = |value: f32| value.to_string().parse().unwrap_or(f64::from(value));
        Ok(lettuce_transfer::build_persona_uec(&PersonaUecSource {
            id: persona.id.to_string(),
            title: persona.title.clone(),
            description: persona.description.clone(),
            nickname: persona.nickname.clone(),
            is_default,
            created_at: persona.created_at.get(),
            updated_at: persona.updated_at.get(),
            avatar: persona
                .media
                .links
                .iter()
                .find(|link| link.slot == PersonaMediaSlot::Avatar)
                .and_then(|link| inline_image(self.media_store, link.asset_id)),
            avatar_crop: persona.avatar_crop.map(|crop| PackageCrop {
                x: decimal(crop.x),
                y: decimal(crop.y),
                scale: decimal(crop.scale),
            }),
            active_lorebook_ids: bindings
                .iter()
                .filter(|binding| binding.enabled)
                .map(|binding| binding.lorebook_id.to_string())
                .collect(),
        })?)
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
        let exported = files.export(persona.id).expect("export");
        let again = files
            .read(&exported, TimestampMillis::new(20))
            .expect("read export");
        assert_eq!(again.persona.title, "Mira");
        assert_eq!(again.persona.nickname.as_deref(), Some("Mi"));
        assert_eq!(again.persona.is_default, Some(true));
        assert_eq!(again.avatar_data.as_deref(), Some(avatar.as_str()));
        assert_eq!(
            again.persona.active_lorebook_ids,
            vec![lorebook.book.id.to_string()]
        );
        assert_eq!(
            again.persona.avatar_crop.as_ref().map(|crop| crop.x),
            Some(0.3)
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
