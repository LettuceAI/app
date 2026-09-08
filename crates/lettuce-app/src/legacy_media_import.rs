use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Component, Path},
};

use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
    MAX_MEDIA_BLOB_BYTES, MediaAssetRepository, MediaBlobRepository, MediaStoreError,
    RetentionClass,
};
use lettuce_transfer::{
    LegacyImportAdmission, LegacyImportAssignment, LegacyImportMediaCompletion,
    LegacyImportMediaCompletionRequest, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyImportRunStatus, LegacyMediaCandidate, LegacyMediaPlan, LegacyMediaUse,
};
use lettuce_types::{AssetId, ContentHash, TimestampMillis};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LegacyMediaImportError {
    #[error("legacy media admission does not match the plan")]
    InvalidAdmission,
    #[error("legacy media source is unavailable")]
    SourceUnavailable,
    #[error("legacy media source path is unsafe")]
    UnsafeSource,
    #[error("legacy media source changed after planning")]
    SourceChanged,
    #[error("legacy media source could not be read")]
    SourceRead,
    #[error("legacy media ingest failed: {0}")]
    Media(MediaStoreError),
    #[error("legacy media completion failed: {0}")]
    Repository(LegacyImportRepositoryError),
}

#[derive(Debug)]
pub struct LegacyMediaImportCoordinator<'a, R: ?Sized, BR, AR> {
    repository: &'a R,
    media_store: &'a LocalMediaBlobStore<BR, AR>,
}

impl<'a, R, BR, AR> LegacyMediaImportCoordinator<'a, R, BR, AR>
where
    R: LegacyImportRepository + ?Sized,
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    #[must_use]
    pub const fn new(repository: &'a R, media_store: &'a LocalMediaBlobStore<BR, AR>) -> Self {
        Self {
            repository,
            media_store,
        }
    }

    pub fn execute(
        &self,
        storage_root: impl AsRef<Path>,
        admission: &LegacyImportAdmission,
        plan: &LegacyMediaPlan,
        completed_at: TimestampMillis,
    ) -> Result<Vec<LegacyImportMediaCompletion>, LegacyMediaImportError> {
        if !matches!(
            admission.status,
            LegacyImportRunStatus::Admitted | LegacyImportRunStatus::Importing
        ) {
            return Err(LegacyMediaImportError::InvalidAdmission);
        }
        let assignments = media_assignments(admission, plan)?;
        let storage_root = std::fs::canonicalize(storage_root)
            .map_err(|_| LegacyMediaImportError::SourceUnavailable)?;
        if !storage_root.is_dir() {
            return Err(LegacyMediaImportError::SourceUnavailable);
        }
        let mut completions = Vec::with_capacity(plan.media.len());
        for candidate in &plan.media {
            let destination_asset_id = assignments
                .get(candidate.relative_path.as_str())
                .copied()
                .ok_or(LegacyMediaImportError::InvalidAdmission)?;
            let bytes = read_verified_source(&storage_root, candidate)?;
            let ingested = self
                .media_store
                .ingest_with_id(
                    destination_asset_id,
                    bytes.as_slice(),
                    IngestRequest::new(
                        asset_kind(candidate),
                        AssetOrigin::Legacy,
                        RetentionClass::Persistent,
                        AssetProvenanceV1 {
                            source_label: Some("Legacy import".to_owned()),
                            imported_format: Some("lettuceai-v92".to_owned()),
                            ..AssetProvenanceV1::default()
                        },
                    ),
                )
                .map_err(LegacyMediaImportError::Media)?;
            if ingested.asset.id != destination_asset_id
                || ingested.blob.content_hash != candidate.content_hash
                || ingested.blob.byte_size != candidate.byte_len
            {
                return Err(LegacyMediaImportError::SourceChanged);
            }
            completions.push(
                self.repository
                    .complete_media(LegacyImportMediaCompletionRequest {
                        run_id: admission.run_id,
                        relative_path: candidate.relative_path.clone(),
                        destination_asset_id,
                        blob_id: ingested.blob.id,
                        byte_len: candidate.byte_len,
                        content_hash: candidate.content_hash.clone(),
                        completed_at,
                    })
                    .map_err(LegacyMediaImportError::Repository)?,
            );
        }
        Ok(completions)
    }
}

fn media_assignments(
    admission: &LegacyImportAdmission,
    plan: &LegacyMediaPlan,
) -> Result<BTreeMap<String, AssetId>, LegacyMediaImportError> {
    let mut expected = BTreeMap::new();
    for candidate in &plan.media {
        if expected
            .insert(
                candidate.relative_path.as_str(),
                (candidate.byte_len, &candidate.content_hash),
            )
            .is_some()
        {
            return Err(LegacyMediaImportError::InvalidAdmission);
        }
    }
    let mut assignments = BTreeMap::new();
    for assignment in &admission.assignments {
        if let LegacyImportAssignment::Media {
            relative_path,
            destination_id,
            byte_len,
            content_hash,
        } = assignment
        {
            let (expected_len, expected_hash) = expected
                .get(relative_path.as_str())
                .ok_or(LegacyMediaImportError::InvalidAdmission)?;
            if *expected_len != *byte_len || **expected_hash != *content_hash {
                return Err(LegacyMediaImportError::InvalidAdmission);
            }
            if assignments
                .insert(relative_path.clone(), *destination_id)
                .is_some()
            {
                return Err(LegacyMediaImportError::InvalidAdmission);
            }
        }
    }
    if assignments.len() != plan.media.len() {
        return Err(LegacyMediaImportError::InvalidAdmission);
    }
    Ok(assignments)
}

fn read_verified_source(
    storage_root: &Path,
    candidate: &LegacyMediaCandidate,
) -> Result<Vec<u8>, LegacyMediaImportError> {
    if candidate.byte_len > MAX_MEDIA_BLOB_BYTES {
        return Err(LegacyMediaImportError::SourceChanged);
    }
    let relative = Path::new(&candidate.relative_path);
    if relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(LegacyMediaImportError::UnsafeSource);
    }
    let mut source = storage_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(LegacyMediaImportError::UnsafeSource);
        };
        source.push(component);
        let metadata = std::fs::symlink_metadata(&source)
            .map_err(|_| LegacyMediaImportError::SourceUnavailable)?;
        if metadata.file_type().is_symlink() {
            return Err(LegacyMediaImportError::UnsafeSource);
        }
    }
    let canonical =
        std::fs::canonicalize(&source).map_err(|_| LegacyMediaImportError::SourceUnavailable)?;
    if !canonical.starts_with(storage_root) || !canonical.is_file() {
        return Err(LegacyMediaImportError::UnsafeSource);
    }
    let mut file = File::open(&canonical).map_err(|_| LegacyMediaImportError::SourceRead)?;
    let metadata = file
        .metadata()
        .map_err(|_| LegacyMediaImportError::SourceRead)?;
    if metadata.len() != candidate.byte_len {
        return Err(LegacyMediaImportError::SourceChanged);
    }
    let capacity =
        usize::try_from(candidate.byte_len).map_err(|_| LegacyMediaImportError::SourceChanged)?;
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(candidate.byte_len.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| LegacyMediaImportError::SourceRead)?;
    if bytes.len() as u64 != candidate.byte_len {
        return Err(LegacyMediaImportError::SourceChanged);
    }
    let content_hash = ContentHash::parse(blake3::hash(&bytes).to_hex().to_string())
        .expect("BLAKE3 always produces a valid content hash");
    if content_hash != candidate.content_hash {
        return Err(LegacyMediaImportError::SourceChanged);
    }
    Ok(bytes)
}

fn asset_kind(candidate: &LegacyMediaCandidate) -> AssetKind {
    let all_persona_avatars = candidate
        .uses
        .iter()
        .all(|media_use| matches!(media_use, LegacyMediaUse::PersonaAvatar { .. }));
    let all_design_references = candidate
        .uses
        .iter()
        .all(|media_use| matches!(media_use, LegacyMediaUse::PersonaDesignReference { .. }));
    let all_lorebook_avatars = candidate
        .uses
        .iter()
        .all(|media_use| matches!(media_use, LegacyMediaUse::LorebookAvatar { .. }));
    if !candidate.uses.is_empty() && all_persona_avatars {
        AssetKind::AvatarOriginal
    } else if !candidate.uses.is_empty() && all_design_references {
        AssetKind::Illustration
    } else if !candidate.uses.is_empty() && all_lorebook_avatars {
        AssetKind::LorebookIcon
    } else {
        AssetKind::OtherImage
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lettuce_characters::{PersonaMediaSlot, PersonaRepository};
    use lettuce_context::{
        DetectionPolicy, KeywordMatchMode, LorebookBehaviorVersion, LorebookRepository,
        PersonaLorebookBindingRepository,
    };
    use lettuce_database::Database;
    use lettuce_media::{LocalMediaBlobStore, MediaAssetRepository, MediaBlobRepository};
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_settings::InMemorySecretStore;
    use lettuce_transfer::{
        LegacyCrop, LegacyDatabaseInventory, LegacyImageRecommendation, LegacyImportAssignment,
        LegacyImportPlan, LegacyImportRunStatus, LegacyKeywordMatchMode, LegacyLorebookCandidate,
        LegacyLorebookDetectionPolicy, LegacyLorebookEntryCandidate, LegacyLorebookPlan,
        LegacyMediaCandidate, LegacyMediaPlan, LegacyMediaReference, LegacyMediaUse,
        LegacyPersonaCandidate, LegacyPersonaPlan, LegacyProviderModelPlan,
    };
    use lettuce_types::{
        ContentHash, LegacyImportRunId, LorebookEntryId, LorebookId, PersonaId, TimestampMillis,
    };

    use crate::{AppBackend, LegacyMediaImportError};

    fn png_fixture(marker: u8) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&2_u32.to_be_bytes());
        bytes.extend_from_slice(&3_u32.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(b"legacy-image");
        bytes.push(marker);
        bytes
    }

    fn content_hash(bytes: &[u8]) -> ContentHash {
        ContentHash::parse(blake3::hash(bytes).to_hex().to_string()).expect("content hash")
    }

    fn provider_models() -> LegacyProviderModelPlan {
        LegacyProviderModelPlan {
            provider_accounts: Vec::new(),
            model_profiles: Vec::new(),
            default_provider_account_id: None,
            default_model_profile_id: None,
        }
    }

    fn import_plan(
        personas: &LegacyPersonaPlan,
        lorebooks: &LegacyLorebookPlan,
        media: &LegacyMediaPlan,
    ) -> LegacyImportPlan {
        LegacyImportPlan {
            provider_models: provider_models(),
            personas: personas.clone(),
            lorebooks: lorebooks.clone(),
            media: media.clone(),
        }
    }

    fn import_plans(
        persona_id: PersonaId,
        bytes: &[u8],
    ) -> (
        LegacyDatabaseInventory,
        LegacyPersonaPlan,
        LegacyLorebookPlan,
        LegacyMediaPlan,
    ) {
        let inventory = LegacyDatabaseInventory {
            schema_version: 92,
            provider_accounts: 0,
            models: 0,
            prompts: 0,
            personas: 1,
            characters: 0,
            lorebooks: 0,
            chat_templates: 0,
            direct_conversations: 0,
            group_profiles: 0,
            group_conversations: 0,
        };
        let personas = LegacyPersonaPlan {
            personas: vec![LegacyPersonaCandidate {
                id: persona_id,
                title: "Owner".to_owned(),
                description: "Imported owner profile".to_owned(),
                nickname: None,
                avatar: None,
                avatar_crop: None,
                design_description: None,
                design_references: Vec::new(),
                image_recommendation: None,
                active_lorebook_ids: Vec::new(),
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(2),
            }],
            default_persona_id: Some(persona_id),
        };
        let hash = content_hash(bytes);
        let media = LegacyMediaPlan {
            media: vec![
                LegacyMediaCandidate {
                    relative_path: "images/avatar.png".to_owned(),
                    byte_len: bytes.len() as u64,
                    content_hash: hash.clone(),
                    uses: vec![LegacyMediaUse::PersonaAvatar { persona_id }],
                },
                LegacyMediaCandidate {
                    relative_path: "images/reference.png".to_owned(),
                    byte_len: bytes.len() as u64,
                    content_hash: hash,
                    uses: vec![LegacyMediaUse::PersonaDesignReference {
                        persona_id,
                        ordinal: 0,
                    }],
                },
            ],
            total_bytes: (bytes.len() as u64) * 2,
        };
        (
            inventory,
            personas,
            LegacyLorebookPlan {
                lorebooks: Vec::new(),
            },
            media,
        )
    }

    fn media_store(
        database_path: &std::path::Path,
        destination_root: &std::path::Path,
    ) -> LocalMediaBlobStore<Database, Database> {
        let snapshot = DirectorySnapshot::new(destination_root).expect("directory snapshot");
        let authority = FilesystemAuthority::new(&snapshot).expect("filesystem authority");
        LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read capability"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write capability"),
            Database::open(database_path).expect("blob database"),
            Database::open(database_path).expect("asset database"),
        )
    }

    #[test]
    fn media_import_deduplicates_blobs_and_replays_after_reopen() {
        let root = std::env::temp_dir().join(format!(
            "lettuce-legacy-media-import-{}",
            LegacyImportRunId::new()
        ));
        let legacy_root = root.join("legacy");
        let images = legacy_root.join("images");
        fs::create_dir_all(&images).expect("create legacy images");
        let bytes = png_fixture(1);
        fs::write(images.join("avatar.png"), &bytes).expect("write avatar");
        fs::write(images.join("reference.png"), &bytes).expect("write reference");
        let database_path = root.join("app.sqlite3");
        let destination_root = root.join("destination");
        let run_id = LegacyImportRunId::new();
        let (inventory, personas, lorebooks, media) = import_plans(PersonaId::new(), &bytes);

        let backend =
            AppBackend::open(&database_path, TimestampMillis::new(10)).expect("open application");
        let admission = backend
            .legacy_import_admission()
            .admit(
                run_id,
                &inventory,
                &import_plan(&personas, &lorebooks, &media),
                TimestampMillis::new(20),
            )
            .expect("admit import");
        let store = media_store(&database_path, &destination_root);
        let first = backend
            .legacy_media_importer(&store)
            .execute(&legacy_root, &admission, &media, TimestampMillis::new(30))
            .expect("import media");
        assert_eq!(first.len(), 2);
        assert_ne!(first[0].destination_asset_id, first[1].destination_asset_id);
        assert_eq!(first[0].blob_id, first[1].blob_id);
        assert!(first.iter().all(|completion| !completion.replayed));
        let replay = backend
            .legacy_media_importer(&store)
            .execute(&legacy_root, &admission, &media, TimestampMillis::new(40))
            .expect("replay media");
        assert!(replay.iter().all(|completion| completion.replayed));
        assert_eq!(
            replay
                .iter()
                .map(|completion| completion.destination_asset_id)
                .collect::<Vec<_>>(),
            first
                .iter()
                .map(|completion| completion.destination_asset_id)
                .collect::<Vec<_>>()
        );
        drop(store);
        drop(backend);

        let reopened =
            AppBackend::open(&database_path, TimestampMillis::new(50)).expect("reopen application");
        let reopened_admission = reopened
            .legacy_import_admission()
            .admit(
                run_id,
                &inventory,
                &import_plan(&personas, &lorebooks, &media),
                TimestampMillis::new(60),
            )
            .expect("replay admission");
        assert_eq!(reopened_admission.status, LegacyImportRunStatus::Importing);
        let reopened_store = media_store(&database_path, &destination_root);
        let reopened_replay = reopened
            .legacy_media_importer(&reopened_store)
            .execute(
                &legacy_root,
                &reopened_admission,
                &media,
                TimestampMillis::new(70),
            )
            .expect("replay after reopen");
        assert!(reopened_replay.iter().all(|completion| completion.replayed));
        let first_asset =
            MediaAssetRepository::get(reopened.database(), reopened_replay[0].destination_asset_id)
                .expect("read first asset")
                .expect("first asset");
        let second_asset =
            MediaAssetRepository::get(reopened.database(), reopened_replay[1].destination_asset_id)
                .expect("read second asset")
                .expect("second asset");
        assert_eq!(first_asset.blob_id, second_asset.blob_id);
        assert_eq!(
            MediaBlobRepository::find_by_hash(reopened.database(), &content_hash(&bytes))
                .expect("read blob")
                .expect("shared blob")
                .id,
            first_asset.blob_id
        );
        assert_eq!(
            fs::read(images.join("avatar.png")).expect("read source"),
            bytes
        );
        drop(reopened_store);
        drop(reopened);
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn partial_media_import_replays_progress_and_rejects_source_drift() {
        let root = std::env::temp_dir().join(format!(
            "lettuce-legacy-media-partial-{}",
            LegacyImportRunId::new()
        ));
        let legacy_root = root.join("legacy");
        let images = legacy_root.join("images");
        fs::create_dir_all(&images).expect("create legacy images");
        let bytes = png_fixture(2);
        fs::write(images.join("avatar.png"), &bytes).expect("write avatar");
        fs::write(images.join("reference.png"), &bytes).expect("write reference");
        let database_path = root.join("app.sqlite3");
        let destination_root = root.join("destination");
        let run_id = LegacyImportRunId::new();
        let (inventory, personas, lorebooks, media) = import_plans(PersonaId::new(), &bytes);
        let backend =
            AppBackend::open(&database_path, TimestampMillis::new(10)).expect("open application");
        let admission = backend
            .legacy_import_admission()
            .admit(
                run_id,
                &inventory,
                &import_plan(&personas, &lorebooks, &media),
                TimestampMillis::new(20),
            )
            .expect("admit import");
        let store = media_store(&database_path, &destination_root);
        fs::remove_file(images.join("reference.png")).expect("remove second source");
        assert_eq!(
            backend.legacy_media_importer(&store).execute(
                &legacy_root,
                &admission,
                &media,
                TimestampMillis::new(30),
            ),
            Err(LegacyMediaImportError::SourceUnavailable)
        );
        fs::write(images.join("reference.png"), &bytes).expect("restore second source");
        let resumed = backend
            .legacy_media_importer(&store)
            .execute(&legacy_root, &admission, &media, TimestampMillis::new(40))
            .expect("resume import");
        assert!(resumed[0].replayed);
        assert!(!resumed[1].replayed);

        let mut changed = bytes.clone();
        let last = changed.last_mut().expect("fixture byte");
        *last ^= 1;
        fs::write(images.join("avatar.png"), &changed).expect("change source");
        assert_eq!(
            backend.legacy_media_importer(&store).execute(
                &legacy_root,
                &admission,
                &media,
                TimestampMillis::new(50),
            ),
            Err(LegacyMediaImportError::SourceChanged)
        );
        assert!(images.join("avatar.png").exists());
        drop(store);
        drop(backend);
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[tokio::test]
    async fn graph_import_preserves_fields_order_and_shared_media_then_replays_after_reopen() {
        let root = std::env::temp_dir().join(format!(
            "lettuce-legacy-graph-import-{}",
            LegacyImportRunId::new()
        ));
        let legacy_root = root.join("legacy");
        let images = legacy_root.join("images");
        fs::create_dir_all(&images).expect("create legacy images");
        let bytes = png_fixture(3);
        fs::write(images.join("shared.png"), &bytes).expect("write shared image");
        fs::write(images.join("reference.png"), &bytes).expect("write reference image");
        let database_path = root.join("app.sqlite3");
        let destination_root = root.join("destination");
        let run_id = LegacyImportRunId::new();
        let persona_id = PersonaId::new();
        let first_book_id = LorebookId::new();
        let second_book_id = LorebookId::new();
        let first_entry_id = LorebookEntryId::new();
        let second_entry_id = LorebookEntryId::new();
        let shared = LegacyMediaReference {
            locator: "shared.png".to_owned(),
        };
        let reference = LegacyMediaReference {
            locator: "reference.png".to_owned(),
        };
        let personas = LegacyPersonaPlan {
            personas: vec![LegacyPersonaCandidate {
                id: persona_id,
                title: "Imported Owner".to_owned(),
                description: "Every persona field survives import.".to_owned(),
                nickname: Some("Owner Nickname".to_owned()),
                avatar: Some(shared.clone()),
                avatar_crop: Some(LegacyCrop {
                    x: 0.25,
                    y: 0.5,
                    scale: 1.75,
                }),
                design_description: Some("Warm studio portrait".to_owned()),
                design_references: vec![reference.clone()],
                image_recommendation: Some(LegacyImageRecommendation {
                    model_name: "Legacy Portrait Model".to_owned(),
                    strength: 0.65,
                }),
                active_lorebook_ids: vec![second_book_id, first_book_id],
                created_at: TimestampMillis::new(11),
                updated_at: TimestampMillis::new(19),
            }],
            default_persona_id: Some(persona_id),
        };
        let lorebooks = LegacyLorebookPlan {
            lorebooks: vec![
                LegacyLorebookCandidate {
                    id: first_book_id,
                    name: "Recent Context".to_owned(),
                    avatar: Some(shared),
                    detection_policy: LegacyLorebookDetectionPolicy::RecentMessageWindow,
                    entries: vec![LegacyLorebookEntryCandidate {
                        id: first_entry_id,
                        title: "Regex Entry".to_owned(),
                        enabled: true,
                        always_active: false,
                        keywords: vec!["city.*".to_owned(), "harbor".to_owned()],
                        case_sensitive: true,
                        match_mode: LegacyKeywordMatchMode::Regex,
                        content: "The harbor closes at dusk.".to_owned(),
                        priority: 9,
                        display_order: 4,
                        created_at: TimestampMillis::new(12),
                        updated_at: TimestampMillis::new(18),
                    }],
                    created_at: TimestampMillis::new(10),
                    updated_at: TimestampMillis::new(20),
                },
                LegacyLorebookCandidate {
                    id: second_book_id,
                    name: "Latest User Context".to_owned(),
                    avatar: None,
                    detection_policy: LegacyLorebookDetectionPolicy::LatestUserMessage,
                    entries: vec![LegacyLorebookEntryCandidate {
                        id: second_entry_id,
                        title: "Always Entry".to_owned(),
                        enabled: false,
                        always_active: true,
                        keywords: Vec::new(),
                        case_sensitive: false,
                        match_mode: LegacyKeywordMatchMode::Literal,
                        content: "A persistent setting detail.".to_owned(),
                        priority: -3,
                        display_order: 7,
                        created_at: TimestampMillis::new(13),
                        updated_at: TimestampMillis::new(17),
                    }],
                    created_at: TimestampMillis::new(9),
                    updated_at: TimestampMillis::new(21),
                },
            ],
        };
        let media = LegacyMediaPlan {
            media: vec![
                LegacyMediaCandidate {
                    relative_path: "images/shared.png".to_owned(),
                    byte_len: bytes.len() as u64,
                    content_hash: content_hash(&bytes),
                    uses: vec![
                        LegacyMediaUse::PersonaAvatar { persona_id },
                        LegacyMediaUse::LorebookAvatar {
                            lorebook_id: first_book_id,
                        },
                    ],
                },
                LegacyMediaCandidate {
                    relative_path: "images/reference.png".to_owned(),
                    byte_len: bytes.len() as u64,
                    content_hash: content_hash(&bytes),
                    uses: vec![LegacyMediaUse::PersonaDesignReference {
                        persona_id,
                        ordinal: 0,
                    }],
                },
            ],
            total_bytes: (bytes.len() * 2) as u64,
        };
        let inventory = LegacyDatabaseInventory {
            schema_version: 92,
            provider_accounts: 0,
            models: 0,
            prompts: 0,
            personas: 1,
            characters: 0,
            lorebooks: 2,
            chat_templates: 0,
            direct_conversations: 0,
            group_profiles: 0,
            group_conversations: 0,
        };
        let backend =
            AppBackend::open(&database_path, TimestampMillis::new(1)).expect("open backend");
        let admission = backend
            .legacy_import_admission()
            .admit(
                run_id,
                &inventory,
                &import_plan(&personas, &lorebooks, &media),
                TimestampMillis::new(30),
            )
            .expect("admit graph");
        let destination_persona_id = admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::Persona {
                    legacy_id,
                    destination_id,
                } if *legacy_id == persona_id => Some(*destination_id),
                _ => None,
            })
            .expect("persona assignment");
        let destination_books = admission
            .assignments
            .iter()
            .filter_map(|assignment| match assignment {
                LegacyImportAssignment::Lorebook {
                    legacy_id,
                    destination_id,
                } => Some((*legacy_id, *destination_id)),
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let destination_entries = admission
            .assignments
            .iter()
            .filter_map(|assignment| match assignment {
                LegacyImportAssignment::LorebookEntry {
                    legacy_id,
                    destination_id,
                } => Some((*legacy_id, *destination_id)),
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            backend.legacy_import_executor().execute(
                &admission,
                &provider_models(),
                &personas,
                &lorebooks,
                &media,
                TimestampMillis::new(35),
            ),
            Err(lettuce_transfer::LegacyImportRepositoryError::Conflict)
        );
        assert!(
            PersonaRepository::get(backend.database(), destination_persona_id)
                .expect("read absent persona")
                .is_none()
        );
        assert!(
            LorebookRepository::get(backend.database(), destination_books[&first_book_id])
                .expect("read absent lorebook")
                .is_none()
        );
        let mut changed_personas = personas.clone();
        changed_personas.personas[0].description = "Changed after admission".to_owned();
        assert_eq!(
            backend.legacy_import_executor().execute(
                &admission,
                &provider_models(),
                &changed_personas,
                &lorebooks,
                &media,
                TimestampMillis::new(36),
            ),
            Err(lettuce_transfer::LegacyImportRepositoryError::Conflict)
        );
        let store = media_store(&database_path, &destination_root);
        backend
            .legacy_media_importer(&store)
            .execute(&legacy_root, &admission, &media, TimestampMillis::new(40))
            .expect("import graph media");
        let receipt = backend
            .legacy_import_executor()
            .execute(
                &admission,
                &provider_models(),
                &personas,
                &lorebooks,
                &media,
                TimestampMillis::new(50),
            )
            .expect("materialize graph");
        assert_eq!(
            (
                receipt.persona_count,
                receipt.lorebook_count,
                receipt.lorebook_entry_count
            ),
            (1, 2, 2)
        );
        assert!(!receipt.replayed);
        let persona = PersonaRepository::get(backend.database(), destination_persona_id)
            .expect("read persona")
            .expect("persona");
        assert_eq!(persona.title, "Imported Owner");
        assert_eq!(persona.nickname.as_deref(), Some("Owner Nickname"));
        assert_eq!(persona.description, "Every persona field survives import.");
        assert_eq!(
            persona.design_description.as_deref(),
            Some("Warm studio portrait")
        );
        let crop = persona.avatar_crop.expect("crop");
        assert_eq!((crop.x, crop.y, crop.scale), (0.25, 0.5, 1.75));
        let recommendation = persona.image_recommendation.expect("recommendation");
        assert_eq!(
            recommendation.unresolved_legacy_name.as_deref(),
            Some("Legacy Portrait Model")
        );
        assert_eq!(recommendation.strength, 0.65);
        assert_eq!(persona.media.links.len(), 2);
        assert_eq!(persona.media.links[0].slot, PersonaMediaSlot::Avatar);
        assert_eq!(persona.media.links[0].ordinal, 0);
        assert_eq!(
            persona.media.links[1].slot,
            PersonaMediaSlot::DesignReference
        );
        assert_eq!(persona.media.links[1].ordinal, 0);
        assert_eq!(
            (persona.created_at, persona.updated_at),
            (TimestampMillis::new(11), TimestampMillis::new(19))
        );
        let first_book =
            LorebookRepository::get(backend.database(), destination_books[&first_book_id])
                .expect("read first lorebook")
                .expect("first lorebook");
        assert_eq!(first_book.book.name, "Recent Context");
        assert_eq!(
            first_book.book.detection_policy,
            DetectionPolicy::RecentMessageWindow
        );
        assert_eq!(
            first_book.book.behavior_version,
            LorebookBehaviorVersion::LegacyV1
        );
        assert_eq!(
            (first_book.book.created_at, first_book.book.updated_at),
            (TimestampMillis::new(10), TimestampMillis::new(20))
        );
        let first_entry = &first_book.entries[0];
        assert_eq!(first_entry.id, destination_entries[&first_entry_id]);
        assert_eq!(first_entry.title, "Regex Entry");
        assert!(first_entry.enabled);
        assert!(!first_entry.always_active);
        assert_eq!(first_entry.keywords, vec!["city.*", "harbor"]);
        assert!(first_entry.case_sensitive);
        assert_eq!(first_entry.match_mode, KeywordMatchMode::Regex);
        assert_eq!(first_entry.content, "The harbor closes at dusk.");
        assert_eq!(first_entry.priority, 9);
        assert_eq!(first_entry.ordinal, 0);
        assert_eq!(
            (first_entry.created_at, first_entry.updated_at),
            (TimestampMillis::new(12), TimestampMillis::new(18))
        );
        assert_eq!(
            first_book.book.icon_asset_id,
            Some(persona.media.links[0].asset_id)
        );
        let second_book =
            LorebookRepository::get(backend.database(), destination_books[&second_book_id])
                .expect("read second lorebook")
                .expect("second lorebook");
        assert_eq!(second_book.book.name, "Latest User Context");
        assert_eq!(
            second_book.book.detection_policy,
            DetectionPolicy::LatestUserMessage
        );
        assert_eq!(second_book.book.icon_asset_id, None);
        let second_entry = &second_book.entries[0];
        assert_eq!(second_entry.id, destination_entries[&second_entry_id]);
        assert_eq!(second_entry.title, "Always Entry");
        assert!(!second_entry.enabled);
        assert!(second_entry.always_active);
        assert!(second_entry.keywords.is_empty());
        assert!(!second_entry.case_sensitive);
        assert_eq!(second_entry.match_mode, KeywordMatchMode::Literal);
        assert_eq!(second_entry.content, "A persistent setting detail.");
        assert_eq!(second_entry.priority, -3);
        assert_eq!(second_entry.ordinal, 0);
        assert_eq!(
            (second_entry.created_at, second_entry.updated_at),
            (TimestampMillis::new(13), TimestampMillis::new(17))
        );
        let bindings = PersonaLorebookBindingRepository::list_persona_bindings(
            backend.database(),
            destination_persona_id,
        )
        .expect("read bindings");
        assert_eq!(
            bindings
                .iter()
                .map(|binding| binding.lorebook_id)
                .collect::<Vec<_>>(),
            vec![
                destination_books[&second_book_id],
                destination_books[&first_book_id]
            ]
        );
        assert!(bindings.iter().all(|binding| binding.enabled));
        assert_eq!(
            bindings
                .iter()
                .map(|binding| binding.ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        let default =
            PersonaRepository::get_default_snapshot(backend.database()).expect("read default");
        assert_eq!(default.state.persona_id, Some(destination_persona_id));
        assert_eq!(default.state.revision.get(), 2);
        assert_eq!(
            fs::read(images.join("shared.png")).expect("read retained source"),
            bytes
        );
        let secret_store = InMemorySecretStore::new();
        backend
            .legacy_provider_model_importer(&secret_store)
            .execute(
                &admission,
                &import_plan(&personas, &lorebooks, &media),
                TimestampMillis::new(55),
            )
            .await
            .expect("complete empty provider graph");
        drop(store);
        drop(backend);

        let reopened =
            AppBackend::open(&database_path, TimestampMillis::new(60)).expect("reopen backend");
        let reopened_admission = reopened
            .legacy_import_admission()
            .admit(
                run_id,
                &inventory,
                &import_plan(&personas, &lorebooks, &media),
                TimestampMillis::new(70),
            )
            .expect("replay completed admission");
        assert_eq!(reopened_admission.status, LegacyImportRunStatus::Completed);
        let replay = reopened
            .legacy_import_executor()
            .execute(
                &reopened_admission,
                &provider_models(),
                &personas,
                &lorebooks,
                &media,
                TimestampMillis::new(80),
            )
            .expect("replay completed graph");
        assert!(replay.replayed);
        assert_eq!(replay.completed_at, TimestampMillis::new(50));
        drop(reopened);
        fs::remove_dir_all(root).expect("remove test root");
    }
}
