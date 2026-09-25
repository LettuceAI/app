use base64::Engine;
use lettuce_characters::{CharacterDetails, CharacterMediaSlot, SceneAssetSlot};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
    MediaAssetRepository, MediaBlobRepository, RetentionClass,
};
use lettuce_transfer::{
    CharacterCardError, CharacterExportError, CharacterExportSource, CharacterFileAssets,
    CharacterFileFormat, CharacterFilePlanError, CharacterFileRepository,
    CharacterFileRepositoryError, CharacterPackage, CharacterPlanError, EntityPackageError,
    LegacyBackupConversionNotice, LegacyImportSkip,
};
use lettuce_types::{AssetId, CharacterId, TimestampMillis};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum CharacterFileError {
    #[error(transparent)]
    Package(#[from] EntityPackageError),
    #[error(transparent)]
    Card(#[from] CharacterCardError),
    #[error(transparent)]
    Plan(#[from] CharacterFilePlanError),
    #[error("character file references could not be resolved: {0}")]
    References(#[from] CharacterPlanError),
    #[error("character file storage failed: {0}")]
    Repository(#[from] CharacterFileRepositoryError),
    #[error(transparent)]
    Export(#[from] CharacterExportError),
    #[error("Character not found")]
    NotFound,
}

/// A character file written as a new character.
#[derive(Debug, Clone)]
pub struct ImportedCharacterFile {
    pub character: CharacterDetails,
    pub skipped: Vec<LegacyImportSkip>,
    pub notices: Vec<LegacyBackupConversionNotice>,
}

/// Character files read into new characters and written from stored ones.
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

    /// The package in a character file's bytes; a `.png` card is read from
    /// its embedded card text and the picture itself becomes the avatar.
    pub fn read_file(
        &self,
        filename: &str,
        data: &[u8],
        now: TimestampMillis,
    ) -> Result<CharacterPackage, CharacterFileError> {
        let json = lettuce_transfer::character_import_json(filename, data)?;
        let mut package = self.read(&json, now)?;
        if filename.to_ascii_lowercase().ends_with(".png") {
            package.avatar_data = Some(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(data)
            ));
        }
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
        let mut notices = plan.notices;
        notices.extend(import.notices);
        Ok(ImportedCharacterFile {
            character,
            skipped,
            notices,
        })
    }

    /// The character written as a file of `format`, its images inlined as
    /// data URLs; an image that cannot be read is left out like legacy did.
    pub fn export(
        &self,
        character_id: CharacterId,
        format: CharacterFileFormat,
        now: TimestampMillis,
    ) -> Result<String, CharacterFileError> {
        let record = self
            .repository
            .character_export_record(character_id)?
            .ok_or(CharacterFileError::NotFound)?;
        let character = &record.details.character;
        let linked = |slot: CharacterMediaSlot| {
            character
                .media
                .links
                .iter()
                .find(|link| link.slot == slot)
                .and_then(|link| self.data_url(link.asset_id))
        };
        let source = CharacterExportSource {
            avatar_data: linked(CharacterMediaSlot::AvatarOriginal),
            background_image_data: linked(CharacterMediaSlot::Background),
            scene_backgrounds: record
                .details
                .scenes
                .iter()
                .filter_map(|scene| {
                    let link = scene
                        .assets
                        .iter()
                        .find(|link| link.slot == SceneAssetSlot::Background)?;
                    Some((scene.id, self.data_url(link.asset_id)?))
                })
                .collect(),
            lorebooks: record.lorebooks,
            scheduled_notes: record.scheduled_notes,
            companion_memory: record.companion_memory,
            details: record.details,
        };
        let package = lettuce_transfer::character_package(&source, now.get());
        let character = &source.details.character;
        Ok(lettuce_transfer::export_character_file(
            &package,
            format,
            &character.id.to_string(),
            character.created_at.get(),
            character.updated_at.get(),
        )?)
    }

    fn data_url(&self, asset_id: AssetId) -> Option<String> {
        inline_image(self.media_store, asset_id)
    }

    fn store_image(&self, data: &str, kind: AssetKind) -> Option<AssetId> {
        store_file_image(self.media_store, data, kind, "lettuceai-character-file")
    }
}

/// A stored image as a data URL with its stored mime type.
pub(crate) fn inline_image<BR, AR>(
    media_store: &LocalMediaBlobStore<BR, AR>,
    asset_id: AssetId,
) -> Option<String>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    let mut opened = media_store
        .open_ready(asset_id)
        .map_err(|error| tracing::warn!(%error, "file image could not be read"))
        .ok()?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut opened.reader, &mut bytes)
        .map_err(|error| tracing::warn!(%error, "file image could not be read"))
        .ok()?;
    Some(format!(
        "data:{};base64,{}",
        opened.blob.mime_type,
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Stores a file's inline image (base64 after the first comma); a remote URL
/// or undecodable image is not stored.
pub(crate) fn store_file_image<BR, AR>(
    media_store: &LocalMediaBlobStore<BR, AR>,
    data: &str,
    kind: AssetKind,
    imported_format: &str,
) -> Option<AssetId>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    if data.starts_with("http://") || data.starts_with("https://") {
        return None;
    }
    let encoded = data.split_once(',').map_or(data, |(_, encoded)| encoded);
    let bytes = match base64::engine::general_purpose::STANDARD.decode(encoded) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(%error, "file image is not base64");
            return None;
        }
    };
    match media_store.ingest(
        bytes.as_slice(),
        IngestRequest::new(
            kind,
            AssetOrigin::Import,
            RetentionClass::Persistent,
            AssetProvenanceV1 {
                imported_format: Some(imported_format.to_owned()),
                ..AssetProvenanceV1::default()
            },
        ),
    ) {
        Ok(ingested) => Some(ingested.asset.id),
        Err(error) => {
            tracing::warn!(%error, "file image could not be stored");
            None
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

    fn png_chunk(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let mut bytes = u32::try_from(body.len())
            .expect("chunk length")
            .to_be_bytes()
            .to_vec();
        bytes.extend_from_slice(kind);
        bytes.extend_from_slice(body);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        bytes
    }

    fn card_png(card: &serde_json::Value) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = Vec::new();
        header.extend_from_slice(&2_u32.to_be_bytes());
        header.extend_from_slice(&3_u32.to_be_bytes());
        header.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend(png_chunk(b"IHDR", &header));
        let mut text = b"chara\0".to_vec();
        text.extend_from_slice(
            base64::engine::general_purpose::STANDARD
                .encode(card.to_string())
                .as_bytes(),
        );
        bytes.extend(png_chunk(b"tEXt", &text));
        bytes.extend(png_chunk(b"IEND", &[]));
        bytes
    }

    fn open_store(
        root: &std::path::Path,
        path: &std::path::Path,
    ) -> LocalMediaBlobStore<Database, Database> {
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(root).expect("snapshot"))
            .expect("authority");
        LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write"),
            Database::open(path).expect("blobs"),
            Database::open(path).expect("assets"),
        )
    }

    #[test]
    fn a_png_card_imports_with_the_png_as_its_avatar_like_legacy_preview_from_bytes() {
        let root = std::env::temp_dir().join(format!("character-png-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let path = root.join("state.sqlite3");
        let backend = crate::AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
        let store = open_store(&root, &path);
        let files = backend.character_files(&store);
        let png = card_png(&serde_json::json!({
            "spec": "chara_card_v2",
            "spec_version": "2.0",
            "data": {
                "name": "Mira",
                "description": "A lighthouse keeper",
                "first_mes": "The lamp is lit.",
                "tags": ["coast", "keeper"]
            }
        }));
        let package = files
            .read_file("Mira.PNG", &png, TimestampMillis::new(5))
            .expect("png card");
        assert_eq!(package.character.name, "Mira");
        let expected = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&png)
        );
        assert_eq!(package.avatar_data.as_deref(), Some(expected.as_str()));
        let imported = files
            .import(&package, TimestampMillis::new(5))
            .expect("import");
        let avatar = imported
            .character
            .character
            .media
            .links
            .iter()
            .find(|link| link.slot == CharacterMediaSlot::AvatarOriginal)
            .expect("avatar from the png");
        let stored = inline_image(&store, avatar.asset_id).expect("stored avatar");
        assert_eq!(stored, expected);
        assert!(matches!(
            files.read_file("Mira.png", b"not a png", TimestampMillis::new(5)),
            Err(CharacterFileError::Card(CharacterCardError::InvalidPng))
        ));
        std::fs::remove_dir_all(&root).ok();
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
                        "companionSharedMemory": {
                            "memories": [" Ada loves tea ", 3],
                            "memorySummary": "They met at the harbour",
                            "soulGrowth": [{
                                "id": "fact-1", "category": "traits", "value": "Patient",
                                "kind": "add", "policy": "adaptive", "slot": "temperament",
                                "confidence": 0.9, "evidenceCount": 2, "weight": 0.8,
                                "validFrom": 10, "locked": false, "createdAt": 10
                            }],
                            "relationshipStates": {
                                "__default__": {"closeness": 0.6, "trust": 0.5, "affection": 0.3, "tension": 0.1, "stability": 0.6, "interactionCount": 3, "lastInteractionAt": 90},
                                "00000000-0000-0000-0000-0000000000aa": {"closeness": 0.1, "trust": 0.1, "affection": 0.1, "tension": 0.0, "stability": 0.5, "interactionCount": 1, "lastInteractionAt": 5}
                            },
                            "createdAt": 20,
                            "updatedAt": 30
                        },
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
        assert!(imported.skipped.iter().any(|skip| skip.reason
            == lettuce_transfer::LegacyImportSkipReason::MissingPersona));
        assert!(
            imported
                .notices
                .iter()
                .any(|notice| notice.field == "memory_summary")
        );
        let exported = files
            .export(
                character.character.id,
                CharacterFileFormat::Uec,
                TimestampMillis::new(60),
            )
            .expect("export");
        let package = files
            .read(&exported, TimestampMillis::new(70))
            .expect("read export");
        assert_eq!(package.avatar_data.as_deref(), Some(avatar.as_str()));
        assert_eq!(package.character.lorebooks.len(), 1);
        assert_eq!(package.character.companion_scheduled_notes.len(), 1);
        let shared = package
            .character
            .companion_shared_memory
            .as_ref()
            .expect("shared memory");
        assert_eq!(shared.memories, serde_json::json!(["Ada loves tea"]));
        assert_eq!(shared.soul_growth.as_array().map(Vec::len), Some(1));
        assert_eq!(
            shared.relationship_states["__default__"]["closeness"],
            serde_json::json!(0.6)
        );
        let again = files
            .import(&package, TimestampMillis::new(70))
            .expect("import export")
            .character;
        assert_ne!(again.character.id, character.character.id);
        assert_eq!(again.character.profile, character.character.profile);
        assert_eq!(
            again.character.defaults.companion_soul,
            character.character.defaults.companion_soul
        );
        assert_eq!(
            backend
                .database()
                .list_scheduled_notes(again.character.id)
                .expect("notes")
                .len(),
            1
        );
        let card = files
            .export(
                character.character.id,
                CharacterFileFormat::CharaCardV2,
                TimestampMillis::new(60),
            )
            .expect("card");
        assert!(card.contains("\"chara_card_v2\""));
        assert!(matches!(
            files.export(
                character.character.id,
                CharacterFileFormat::CharaCardV1,
                TimestampMillis::new(60)
            ),
            Err(CharacterFileError::Export(
                CharacterExportError::CardV1ReadOnly
            ))
        ));
        std::fs::remove_dir_all(&root).ok();
    }
}
