use base64::Engine;
use lettuce_characters::CharacterDetails;
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
    MediaAssetRepository, MediaBlobRepository, RetentionClass,
};
use lettuce_transfer::{
    CharacterFileAssets, CharacterFilePlanError, CharacterFileRepository,
    CharacterFileRepositoryError, CharacterPackage, CharacterPlanError, EntityPackageError,
    LegacyBackupConversionNotice, LegacyImportSkip,
};
use lettuce_types::{AssetId, TimestampMillis};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum CharacterFileError {
    #[error(transparent)]
    Package(#[from] EntityPackageError),
    #[error(transparent)]
    Plan(#[from] CharacterFilePlanError),
    #[error("character file references could not be resolved: {0}")]
    References(#[from] CharacterPlanError),
    #[error("character file storage failed: {0}")]
    Repository(#[from] CharacterFileRepositoryError),
}

/// A character file written as a new character.
#[derive(Debug, Clone)]
pub struct ImportedCharacterFile {
    pub character: CharacterDetails,
    pub skipped: Vec<LegacyImportSkip>,
    pub notices: Vec<LegacyBackupConversionNotice>,
}

/// Character files read into new characters.
#[derive(Debug)]
pub struct CharacterFileCoordinator<'a, R: ?Sized, BR, AR> {
    repository: &'a R,
    media_store: &'a LocalMediaBlobStore<BR, AR>,
}

impl<'a, R: ?Sized, BR, AR> CharacterFileCoordinator<'a, R, BR, AR> {
    #[must_use]
    pub const fn new(repository: &'a R, media_store: &'a LocalMediaBlobStore<BR, AR>) -> Self {
        Self {
            repository,
            media_store,
        }
    }
}

impl<R, BR, AR> CharacterFileCoordinator<'_, R, BR, AR>
where
    R: CharacterFileRepository + ?Sized,
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    /// The package in any supported character file; the host may replace a
    /// remote `avatar_data` with the downloaded image's data URL before
    /// importing it.
    pub fn read(
        &self,
        json: &str,
        now: TimestampMillis,
    ) -> Result<CharacterPackage, CharacterFileError> {
        let value = serde_json::from_str(json)
            .map_err(|error| EntityPackageError::InvalidImport(error.to_string()))?;
        let (package, _) = lettuce_transfer::parse_character_import(&value, now.get(), || {
            Uuid::new_v4().to_string()
        })?;
        Ok(package)
    }

    /// Writes the package as a new character with its bundled lorebooks;
    /// an image that cannot be stored is left off like legacy did.
    pub fn import(
        &self,
        package: &CharacterPackage,
        now: TimestampMillis,
    ) -> Result<ImportedCharacterFile, CharacterFileError> {
        let references = self.repository.character_file_references()?;
        let plan =
            lettuce_transfer::plan_character_file(package, &references, now.get(), Uuid::new_v4)?;
        let assets = CharacterFileAssets {
            avatar: plan
                .avatar_data
                .as_deref()
                .and_then(|data| self.store_image(data, AssetKind::AvatarOriginal)),
            background: plan
                .background_image_data
                .as_deref()
                .and_then(|data| self.store_image(data, AssetKind::BackgroundImage)),
            scene_backgrounds: plan
                .scene_backgrounds
                .iter()
                .filter_map(|(scene_id, data)| {
                    Some((
                        *scene_id,
                        self.store_image(data, AssetKind::BackgroundImage)?,
                    ))
                })
                .collect(),
        };
        let import = plan.import(&references, &assets, now.get(), Uuid::new_v4)?;
        let character = self.repository.import_character_file(&import)?;
        let mut skipped = plan.skipped;
        skipped.extend(import.skipped);
        Ok(ImportedCharacterFile {
            character,
            skipped,
            notices: plan.notices,
        })
    }

    fn store_image(&self, data: &str, kind: AssetKind) -> Option<AssetId> {
        if data.starts_with("http://") || data.starts_with("https://") {
            return None;
        }
        let encoded = data.split_once(',').map_or(data, |(_, encoded)| encoded);
        let bytes = match base64::engine::general_purpose::STANDARD.decode(encoded) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(%error, "character file image is not base64");
                return None;
            }
        };
        match self.media_store.ingest(
            bytes.as_slice(),
            IngestRequest::new(
                kind,
                AssetOrigin::Import,
                RetentionClass::Persistent,
                AssetProvenanceV1 {
                    imported_format: Some("lettuceai-character-file".to_owned()),
                    ..AssetProvenanceV1::default()
                },
            ),
        ) {
            Ok(ingested) => Some(ingested.asset.id),
            Err(error) => {
                tracing::warn!(%error, "character file image could not be stored");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use lettuce_characters::CharacterMediaSlot;
    use lettuce_companions::CompanionScheduledNoteRepository;
    use lettuce_context::{CharacterLorebookBindingRepository, LorebookRepository};
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
        bytes.extend_from_slice(b"avatar bytes");
        bytes
    }

    #[test]
    fn a_character_file_becomes_a_new_character_with_its_lorebooks_notes_and_images() {
        let root = std::env::temp_dir().join(format!("character-file-{}", OperationId::new()));
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
        let avatar = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png())
        );
        let files = backend.character_files(&store);
        let package = files
            .read(
                &serde_json::json!({
                    "version": 1,
                    "exportedAt": 5,
                    "character": {
                        "name": "Ada",
                        "description": "Keeper",
                        "rules": [],
                        "scenes": [
                            {"id": "s1", "content": "Harbour", "backgroundImagePath": "data:image/png;base64,@@", "variants": []}
                        ],
                        "mode": "companion",
                        "companion": {"soul": {"essence": "Warm"}},
                        "activeLorebookIds": ["book-1", "00000000-0000-0000-0000-00000000beef"],
                        "lorebooks": [{
                            "lorebook": {"id": "book-1", "name": "Harbour", "avatarPath": null, "keywordDetectionMode": "recentMessageWindow", "createdAt": 1, "updatedAt": 1},
                            "entries": [{"id": "e1", "lorebookId": "book-1", "title": "Key", "enabled": true, "alwaysActive": false, "keywords": ["key"], "caseSensitive": false, "keywordMatchMode": "literal", "content": "Ada keeps it", "priority": 0, "displayOrder": 0, "createdAt": 1, "updatedAt": 1}]
                        }],
                        "companionScheduledNotes": [
                            {"label": " Birthday ", "content": " Wish her well ", "availableAt": 10, "recurrence": "Yearly"},
                            {"label": "Blank", "content": "  ", "availableAt": 10},
                            {"id": "late", "label": "Late", "content": "Too late", "availableAt": 10, "expiresAt": 5}
                        ],
                        "disableAvatarGradient": false
                    },
                    "avatarData": avatar
                })
                .to_string(),
                TimestampMillis::new(50),
            )
            .expect("read");
        let imported = files
            .import(&package, TimestampMillis::new(50))
            .expect("import");
        let character = &imported.character;
        assert_eq!(character.character.profile.name, "Ada");
        assert!(
            character
                .character
                .media
                .links
                .iter()
                .any(|link| link.slot == CharacterMediaSlot::AvatarOriginal)
        );
        assert!(character.scenes[0].assets.is_empty());
        let bindings = backend
            .database()
            .list_character_bindings(character.character.id)
            .expect("bindings");
        assert_eq!(bindings.len(), 1);
        let lorebook = LorebookRepository::get(backend.database(), bindings[0].lorebook_id)
            .expect("lorebook")
            .expect("bundled lorebook");
        assert_eq!(lorebook.book.name, "Harbour");
        assert_eq!(lorebook.entries[0].content, "Ada keeps it");
        let notes = backend
            .database()
            .list_scheduled_notes(character.character.id)
            .expect("notes");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].label, "Birthday");
        assert_eq!(notes[0].content, "Wish her well");
        assert_eq!(
            notes[0].recurrence,
            lettuce_companions::ScheduledNoteRecurrence::Yearly
        );
        assert!(
            imported.skipped.iter().any(
                |skip| skip.reason == lettuce_transfer::LegacyImportSkipReason::MissingLorebook
            )
        );
        assert!(
            imported
                .skipped
                .iter()
                .any(|skip| skip.source_key == "companion_scheduled_notes:late")
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
