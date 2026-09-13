use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use lettuce_transfer::{
    LEGACY_MEDIA_TOTAL_BYTES_LIMIT, LegacyBackupCompatibilityError, LegacyBackupCompatibilityPlan,
    LegacyBackupDocument, LegacyBackupDocumentKind, LegacyBackupInventory, LegacyBackupMedia,
    LegacyBackupMediaRoot, LegacyDatabasePreflightError, LegacyImportPlan, LegacyLorebookPlan,
    LegacyMediaPlan, LegacyPersonaPlan, MAX_BACKUP_ENTRIES, MAX_BACKUP_ENTRY_BYTES,
    MAX_BACKUP_TOTAL_BYTES, plan_legacy_backup_compatibility,
};
use lettuce_types::ContentHash;
use zeroize::Zeroizing;

const STORAGE_MEDIA_ROOTS: [LegacyBackupMediaRoot; 4] = [
    LegacyBackupMediaRoot::Images,
    LegacyBackupMediaRoot::Avatars,
    LegacyBackupMediaRoot::Attachments,
    LegacyBackupMediaRoot::Sessions,
];
const MAX_RELATIVE_PATH_BYTES: usize = 4096;
const LEGACY_DATABASE_APP_VERSION: &str = "legacy-database";

/// Reads the legacy app data directory into the same inventory a legacy
/// backup archive of it decodes to: the database documents plus every file
/// under the media roots the legacy exporter archived.
pub fn read_legacy_database_inventory(
    app_data_dir: impl AsRef<Path>,
) -> Result<LegacyBackupInventory, LegacyDatabasePreflightError> {
    let app_data_dir =
        fs::canonicalize(app_data_dir).map_err(|_| LegacyDatabasePreflightError::Unavailable)?;
    let storage_root = app_data_dir.join("lettuce");
    let documents = lettuce_database::read_legacy_database_documents(storage_root.join("app.db"))?;
    let media = read_legacy_media(&app_data_dir)?;
    let created_at = settings_updated_at(&documents);
    let source_hash = source_hash(created_at, &documents, &media)?;
    Ok(LegacyBackupInventory {
        version: 1,
        created_at,
        app_version: LEGACY_DATABASE_APP_VERSION.into(),
        source_hash,
        documents,
        media,
    })
}

fn read_legacy_media(
    app_data_dir: &Path,
) -> Result<Vec<LegacyBackupMedia>, LegacyDatabasePreflightError> {
    let storage_root = app_data_dir.join("lettuce");
    let mut walk = MediaWalk::default();
    for root in STORAGE_MEDIA_ROOTS {
        walk.directory(
            &storage_root.join(root.archive_name()),
            root,
            &mut Vec::new(),
        )?;
    }
    walk.directory(
        &app_data_dir.join(LegacyBackupMediaRoot::GeneratedImages.archive_name()),
        LegacyBackupMediaRoot::GeneratedImages,
        &mut Vec::new(),
    )?;
    let mut media = walk.media;
    media.sort_by(|left, right| {
        (&left.root, &left.relative_segments).cmp(&(&right.root, &right.relative_segments))
    });
    Ok(media)
}

#[derive(Default)]
struct MediaWalk {
    media: Vec<LegacyBackupMedia>,
    total_bytes: u64,
}

impl MediaWalk {
    fn directory(
        &mut self,
        path: &Path,
        root: LegacyBackupMediaRoot,
        segments: &mut Vec<String>,
    ) -> Result<(), LegacyDatabasePreflightError> {
        let Ok(entries) = fs::read_dir(path) else {
            return Ok(());
        };
        let mut children = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect::<Vec<_>>();
        children.sort();
        for child in children {
            let Some(name) = child.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Ok(metadata) = fs::symlink_metadata(&child) else {
                continue;
            };
            segments.push(name.to_owned());
            if !safe_relative_path(segments) {
                segments.pop();
                continue;
            }
            if metadata.is_dir() {
                self.directory(&child, root, segments)?;
            } else if metadata.is_file() {
                self.file(&child, root, segments, metadata.len())?;
            }
            segments.pop();
        }
        Ok(())
    }

    fn file(
        &mut self,
        path: &Path,
        root: LegacyBackupMediaRoot,
        segments: &[String],
        expected_len: u64,
    ) -> Result<(), LegacyDatabasePreflightError> {
        let object_limit = MAX_BACKUP_ENTRY_BYTES as u64;
        let total_limit = MAX_BACKUP_TOTAL_BYTES as u64;
        if self.media.len() >= MAX_BACKUP_ENTRIES {
            return Err(LegacyDatabasePreflightError::LimitExceeded {
                table: "media_files",
                limit: MAX_BACKUP_ENTRIES as u32,
            });
        }
        if expected_len > object_limit {
            return Err(LegacyDatabasePreflightError::MediaObjectTooLarge {
                locator: locator(root, segments),
                limit: object_limit,
            });
        }
        let mut bytes = Zeroizing::new(Vec::new());
        File::open(path)
            .and_then(|file| file.take(object_limit + 1).read_to_end(&mut bytes))
            .map_err(|_| read_failed(root, segments))?;
        let len = bytes.len() as u64;
        if len > object_limit {
            return Err(LegacyDatabasePreflightError::MediaObjectTooLarge {
                locator: locator(root, segments),
                limit: object_limit,
            });
        }
        self.total_bytes = self
            .total_bytes
            .checked_add(len)
            .filter(|total| *total <= total_limit)
            .ok_or(LegacyDatabasePreflightError::MediaTotalTooLarge { limit: total_limit })?;
        self.media.push(LegacyBackupMedia {
            root,
            relative_segments: segments.to_vec(),
            bytes,
        });
        Ok(())
    }
}

fn safe_relative_path(segments: &[String]) -> bool {
    let bytes = segments
        .iter()
        .map(|segment| segment.len() + 1)
        .sum::<usize>();
    bytes <= MAX_RELATIVE_PATH_BYTES
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && !matches!(segment.as_str(), "." | "..")
                && !segment
                    .chars()
                    .any(|character| character.is_control() || matches!(character, '/' | '\\'))
        })
}

fn locator(root: LegacyBackupMediaRoot, segments: &[String]) -> String {
    let root = root.archive_name();
    if segments.is_empty() {
        root.to_owned()
    } else {
        format!("{root}/{}", segments.join("/"))
    }
}

fn read_failed(root: LegacyBackupMediaRoot, segments: &[String]) -> LegacyDatabasePreflightError {
    LegacyDatabasePreflightError::MediaReadFailed {
        locator: locator(root, segments),
    }
}

fn settings_updated_at(documents: &[LegacyBackupDocument]) -> u64 {
    documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::Settings)
        .and_then(|document| serde_json::from_slice::<serde_json::Value>(&document.bytes).ok())
        .and_then(|settings| {
            settings
                .get("updated_at")
                .and_then(serde_json::Value::as_u64)
        })
        .unwrap_or_default()
}

fn source_hash(
    created_at: u64,
    documents: &[LegacyBackupDocument],
    media: &[LegacyBackupMedia],
) -> Result<ContentHash, LegacyDatabasePreflightError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"lettuce-legacy-database-inventory-v1");
    hasher.update(&created_at.to_le_bytes());
    hasher.update(&(documents.len() as u64).to_le_bytes());
    for document in documents {
        add_text(&mut hasher, document.kind.archive_stem());
        hasher.update(&(document.bytes.len() as u64).to_le_bytes());
        hasher.update(&document.bytes);
    }
    hasher.update(&(media.len() as u64).to_le_bytes());
    for item in media {
        add_text(&mut hasher, item.root.archive_name());
        hasher.update(&(item.relative_segments.len() as u64).to_le_bytes());
        for segment in &item.relative_segments {
            add_text(&mut hasher, segment);
        }
        hasher.update(&(item.bytes.len() as u64).to_le_bytes());
        hasher.update(&item.bytes);
    }
    ContentHash::parse(hasher.finalize().to_hex().to_string())
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)
}

fn add_text(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

#[derive(Debug)]
pub struct LegacyDatabaseImportPlan {
    pub compatibility: LegacyBackupCompatibilityPlan,
    pub plan: LegacyImportPlan,
}

#[derive(Debug, thiserror::Error)]
pub enum LegacyDatabaseImportPlanError {
    #[error("{0}")]
    Database(LegacyDatabasePreflightError),
    #[error(transparent)]
    Compatibility(#[from] LegacyBackupCompatibilityError),
}

impl From<LegacyDatabasePreflightError> for LegacyDatabaseImportPlanError {
    fn from(value: LegacyDatabasePreflightError) -> Self {
        Self::Database(value)
    }
}

/// Plans a live legacy app data directory through the shared legacy backup
/// planner. ASR rows and voice audio come from the database tables, which keep
/// the row ids and voice examples a legacy backup document never carried.
pub fn plan_legacy_database_import(
    app_data_dir: impl AsRef<Path>,
) -> Result<LegacyDatabaseImportPlan, LegacyDatabaseImportPlanError> {
    let app_data_dir =
        fs::canonicalize(app_data_dir).map_err(|_| LegacyDatabasePreflightError::Unavailable)?;
    let storage_root = app_data_dir.join("lettuce");
    let compatibility =
        plan_legacy_backup_compatibility(read_legacy_database_inventory(&app_data_dir)?)?;
    let mut plan = compatibility.legacy_import_plan();
    plan.asr = lettuce_database::plan_legacy_asr(storage_root.join("app.db"))?;
    let voice_audio = crate::plan_legacy_media(
        &storage_root,
        &mut LegacyPersonaPlan {
            personas: Vec::new(),
            default_persona_id: None,
            skipped: Vec::new(),
        },
        &mut LegacyLorebookPlan {
            lorebooks: Vec::new(),
            skipped: Vec::new(),
        },
        &plan.asr,
    )?;
    merge_media(&mut plan.media, voice_audio)?;
    Ok(LegacyDatabaseImportPlan {
        compatibility,
        plan,
    })
}

fn merge_media(
    media: &mut LegacyMediaPlan,
    extra: LegacyMediaPlan,
) -> Result<(), LegacyDatabasePreflightError> {
    media.total_bytes = media
        .total_bytes
        .checked_add(extra.total_bytes)
        .filter(|total| *total <= LEGACY_MEDIA_TOTAL_BYTES_LIMIT)
        .ok_or(LegacyDatabasePreflightError::MediaTotalTooLarge {
            limit: LEGACY_MEDIA_TOTAL_BYTES_LIMIT,
        })?;
    media.media.extend(extra.media);
    media
        .media
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    media.skipped.extend(extra.skipped);
    media.skipped.sort();
    media.skipped.dedup();
    Ok(())
}

#[cfg(test)]
mod tests {
    use lettuce_types::MediaBlobId;

    use super::*;

    fn media_plan(paths: &[&str], total_bytes: u64) -> LegacyMediaPlan {
        LegacyMediaPlan {
            media: paths
                .iter()
                .map(|path| lettuce_transfer::LegacyMediaCandidate {
                    relative_path: (*path).to_owned(),
                    source_locator: (*path).to_owned(),
                    byte_len: 1,
                    content_hash: ContentHash::parse("44".repeat(32)).expect("hash"),
                    uses: Vec::new(),
                })
                .collect(),
            total_bytes,
            skipped: Vec::new(),
        }
    }

    #[test]
    fn voice_audio_merges_into_the_authored_media_plan_in_path_order_within_the_total_limit() {
        let mut media = media_plan(&["images/b.png", "avatars/a/a.png"], 2);
        merge_media(&mut media, media_plan(&["asr/voice-examples/1.wav"], 1)).expect("merge");
        assert_eq!(
            media
                .media
                .iter()
                .map(|item| item.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "asr/voice-examples/1.wav",
                "avatars/a/a.png",
                "images/b.png"
            ]
        );
        assert_eq!(media.total_bytes, 3);

        let mut full = media_plan(&[], LEGACY_MEDIA_TOTAL_BYTES_LIMIT);
        assert_eq!(
            merge_media(&mut full, media_plan(&["asr/voice-examples/2.wav"], 1)),
            Err(LegacyDatabasePreflightError::MediaTotalTooLarge {
                limit: LEGACY_MEDIA_TOTAL_BYTES_LIMIT,
            })
        );
    }

    #[test]
    fn a_data_directory_without_a_legacy_database_is_unavailable() {
        let root = std::env::temp_dir().join(format!("lettuce-legacy-none-{}", MediaBlobId::new()));
        fs::create_dir_all(&root).expect("empty root");
        assert!(matches!(
            plan_legacy_database_import(&root),
            Err(LegacyDatabaseImportPlanError::Database(
                LegacyDatabasePreflightError::Unavailable
            ))
        ));
        fs::remove_dir_all(root).expect("remove fixture");
    }

    fn app_data_dir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("lettuce-legacy-app-{}", MediaBlobId::new()));
        fs::create_dir_all(path.join("lettuce/avatars/character-1")).expect("avatar directory");
        fs::create_dir_all(path.join("lettuce/sessions/session-1")).expect("session directory");
        fs::create_dir_all(path.join("generated_images")).expect("generated directory");
        fs::write(
            path.join("lettuce/avatars/character-1/avatar.webp"),
            b"avatar",
        )
        .expect("avatar");
        fs::write(path.join("lettuce/sessions/session-1/photo.png"), b"photo").expect("photo");
        fs::write(path.join("generated_images/render.png"), b"render").expect("render");
        path
    }

    #[test]
    fn media_walk_collects_every_archived_root_in_archive_order() {
        let root = app_data_dir();

        let media = read_legacy_media(&root).expect("legacy media");

        let found = media
            .iter()
            .map(|item| {
                (
                    item.root,
                    item.relative_segments.join("/"),
                    item.bytes.to_vec(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            found,
            vec![
                (
                    LegacyBackupMediaRoot::Avatars,
                    "character-1/avatar.webp".to_owned(),
                    b"avatar".to_vec()
                ),
                (
                    LegacyBackupMediaRoot::Sessions,
                    "session-1/photo.png".to_owned(),
                    b"photo".to_vec()
                ),
                (
                    LegacyBackupMediaRoot::GeneratedImages,
                    "render.png".to_owned(),
                    b"render".to_vec()
                ),
            ]
        );
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[cfg(unix)]
    #[test]
    fn media_walk_never_follows_symbolic_links_out_of_the_legacy_roots() {
        let root = app_data_dir();
        let outside = root.join("outside.txt");
        fs::write(&outside, b"private").expect("outside file");
        std::os::unix::fs::symlink(&outside, root.join("lettuce/avatars/link.webp"))
            .expect("file link");
        std::os::unix::fs::symlink(root.join("lettuce"), root.join("generated_images/loop"))
            .expect("directory link");

        let media = read_legacy_media(&root).expect("legacy media");

        assert_eq!(media.len(), 3);
        assert!(media.iter().all(|item| item.bytes.as_slice() != b"private"));
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn missing_media_roots_are_empty_and_the_source_hash_binds_media_bytes() {
        let root =
            std::env::temp_dir().join(format!("lettuce-legacy-empty-{}", MediaBlobId::new()));
        fs::create_dir_all(&root).expect("empty root");
        assert!(read_legacy_media(&root).expect("no media").is_empty());

        fs::remove_dir_all(root).expect("remove fixture");

        let fixture = app_data_dir();
        let first = read_legacy_media(&fixture).expect("media");
        let mut changed = read_legacy_media(&fixture).expect("media");
        changed[0].bytes = Zeroizing::new(b"other".to_vec());
        assert_ne!(
            source_hash(1, &[], &first).expect("hash"),
            source_hash(1, &[], &changed).expect("hash")
        );
        assert_eq!(
            source_hash(1, &[], &first).expect("hash"),
            source_hash(1, &[], &read_legacy_media(&fixture).expect("media")).expect("hash")
        );
        fs::remove_dir_all(fixture).expect("remove fixture");
    }

    #[test]
    fn oversized_media_objects_reject_with_their_locator() {
        let root = app_data_dir();
        let file = File::create(root.join("generated_images/huge.bin")).expect("huge file");
        file.set_len(MAX_BACKUP_ENTRY_BYTES as u64 + 1)
            .expect("sparse size");

        let error = read_legacy_media(&root).expect_err("oversized media");

        assert_eq!(
            error,
            LegacyDatabasePreflightError::MediaObjectTooLarge {
                locator: "generated_images/huge.bin".into(),
                limit: MAX_BACKUP_ENTRY_BYTES as u64,
            }
        );
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
