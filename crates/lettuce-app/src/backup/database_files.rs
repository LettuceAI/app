use std::{collections::BTreeSet, time::UNIX_EPOCH};

use lettuce_platform::{ManagedFileLock, ParentSyncStatus};
use lettuce_types::ContentHash;
use serde::{Deserialize, Serialize};

use super::*;

const LIFECYCLE_KEY: &str = "database-file-lifecycle.lock";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseFileKind {
    Initial,
    Restore,
    LegacyRestore,
    Reset,
    Existing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum FileState {
    Writing,
    Abandoned,
    Active,
    Kept,
    Activating { previous: String },
    Keeping { next: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileMetadata {
    version: u32,
    kind: DatabaseFileKind,
    created_at: TimestampMillis,
    kept_at: Option<TimestampMillis>,
    kept_hashes: BTreeSet<ContentHash>,
    state: FileState,
}

#[derive(Debug, Clone)]
pub struct AppDatabaseFile {
    pub file: String,
    pub kind: DatabaseFileKind,
    pub created_at: TimestampMillis,
    pub size: u64,
    pub active: bool,
    pub deletable: bool,
}

#[derive(Debug)]
pub struct DatabaseFileLifecycle {
    location: AppDatabaseLocation,
    _lock: ManagedFileLock,
}

impl AppDatabaseLocation {
    pub async fn file_lifecycle(&self) -> Result<DatabaseFileLifecycle, AppDatabaseLocationError> {
        let location = self.clone();
        tokio::task::spawn_blocking(move || location.acquire_file_lifecycle(true))
            .await
            .map_err(|_| AppDatabaseLocationError::Storage)?
    }

    pub fn try_file_lifecycle(&self) -> Result<DatabaseFileLifecycle, AppDatabaseLocationError> {
        self.acquire_file_lifecycle(false)
    }

    pub(crate) fn acquire_file_lifecycle(
        &self,
        wait: bool,
    ) -> Result<DatabaseFileLifecycle, AppDatabaseLocationError> {
        let lock = self
            .files
            .lock_file(&self.write, &ObjectKey::single(LIFECYCLE_KEY)?, wait)?
            .ok_or(AppDatabaseLocationError::Busy)?;
        let guard = DatabaseFileLifecycle {
            location: self.clone(),
            _lock: lock,
        };
        guard.recover()?;
        Ok(guard)
    }
}

impl DatabaseFileLifecycle {
    fn key(name: &str) -> Result<ObjectKey, AppDatabaseLocationError> {
        Ok(ObjectKey::from_segments([
            DATABASE_DIRECTORY,
            &format!("{name}.meta.json"),
        ])?)
    }

    fn read(&self, name: &str) -> Result<Option<FileMetadata>, AppDatabaseLocationError> {
        self.location.database_path(name)?;
        match self
            .location
            .files
            .read(&self.location.read, &Self::key(name)?)
        {
            Ok(bytes) => {
                let metadata: FileMetadata = serde_json::from_slice(&bytes)
                    .map_err(|_| AppDatabaseLocationError::Corrupt)?;
                if metadata.version != 1
                    || metadata.kept_at.is_none() && !metadata.kept_hashes.is_empty()
                {
                    return Err(AppDatabaseLocationError::Corrupt);
                }
                Ok(Some(metadata))
            }
            Err(PlatformError::NotFound) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn write(&self, name: &str, metadata: &FileMetadata) -> Result<(), AppDatabaseLocationError> {
        self.location.database_path(name)?;
        let bytes = serde_json::to_vec(metadata).map_err(|_| AppDatabaseLocationError::Corrupt)?;
        let receipt =
            self.location
                .files
                .write_atomic(&self.location.write, Self::key(name)?, &bytes)?;
        if receipt.parent_sync == ParentSyncStatus::Failed {
            return Err(AppDatabaseLocationError::Storage);
        }
        Ok(())
    }

    fn names(&self) -> Result<Vec<String>, AppDatabaseLocationError> {
        let directory = self.location.private_persistent.join(DATABASE_DIRECTORY);
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(PlatformError::from(error).into()),
        };
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(PlatformError::from)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| AppDatabaseLocationError::Corrupt)?;
            if !name.ends_with(DATABASE_EXTENSION) {
                continue;
            }
            self.location.database_path(&name)?;
            if !entry.file_type().map_err(PlatformError::from)?.is_file() {
                return Err(AppDatabaseLocationError::Corrupt);
            }
            names.push(name);
        }
        names.sort();
        Ok(names)
    }

    fn capture(&self, name: &str) -> Result<BTreeSet<ContentHash>, AppDatabaseLocationError> {
        Database::media_objects_in_file(&self.location.database_path(name)?)
            .map_err(|_| AppDatabaseLocationError::Storage)
    }

    fn active_name(&self) -> Result<String, AppDatabaseLocationError> {
        self.location
            .active_path()?
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .ok_or(AppDatabaseLocationError::Corrupt)
    }

    fn recover(&self) -> Result<(), AppDatabaseLocationError> {
        let active = self.active_name()?;
        for name in self.names()? {
            let existing = self.read(&name)?;
            let needs_write = existing.as_ref().is_none_or(|metadata| {
                matches!(
                    metadata.state,
                    FileState::Writing | FileState::Activating { .. } | FileState::Keeping { .. }
                )
            });
            let mut metadata = match existing {
                Some(metadata) => metadata,
                None => {
                    let path = self.location.database_path(&name)?;
                    let created = path
                        .metadata()
                        .and_then(|metadata| metadata.created())
                        .map_err(PlatformError::from)?;
                    let millis = created
                        .duration_since(UNIX_EPOCH)
                        .map_err(|_| AppDatabaseLocationError::Corrupt)?
                        .as_millis();
                    let timestamp = TimestampMillis::new(
                        i64::try_from(millis).map_err(|_| AppDatabaseLocationError::Corrupt)?,
                    );
                    FileMetadata {
                        version: 1,
                        kind: if name == active {
                            DatabaseFileKind::Initial
                        } else {
                            DatabaseFileKind::Existing
                        },
                        created_at: timestamp,
                        kept_at: (name != active).then_some(timestamp),
                        kept_hashes: if name == active {
                            BTreeSet::new()
                        } else {
                            self.capture(&name)?
                        },
                        state: if name == active {
                            FileState::Active
                        } else {
                            FileState::Kept
                        },
                    }
                }
            };
            metadata.state = match &metadata.state {
                FileState::Writing => FileState::Abandoned,
                FileState::Activating { .. } if name == active => FileState::Active,
                FileState::Activating { .. } => FileState::Abandoned,
                FileState::Keeping { next } if next == &active => FileState::Kept,
                FileState::Keeping { .. } => {
                    metadata.kept_at = None;
                    metadata.kept_hashes.clear();
                    FileState::Active
                }
                state => state.clone(),
            };
            if matches!(metadata.state, FileState::Active) && name != active {
                return Err(AppDatabaseLocationError::Corrupt);
            }
            if needs_write {
                self.write(&name, &metadata)?;
            }
        }
        Ok(())
    }

    pub(crate) fn begin_file(
        &self,
        name: &str,
        kind: DatabaseFileKind,
        created_at: TimestampMillis,
    ) -> Result<PathBuf, AppDatabaseLocationError> {
        let path = self.location.database_path(name)?;
        if path.try_exists().map_err(PlatformError::from)? || self.read(name)?.is_some() {
            return Err(AppDatabaseLocationError::Exists);
        }
        self.write(
            name,
            &FileMetadata {
                version: 1,
                kind,
                created_at,
                kept_at: None,
                kept_hashes: BTreeSet::new(),
                state: FileState::Writing,
            },
        )?;
        Ok(path)
    }

    pub(super) fn prepare_cutover(
        &self,
        name: &str,
        at: TimestampMillis,
    ) -> Result<(), AppDatabaseLocationError> {
        let previous = self.active_name()?;
        if previous == name {
            return Err(AppDatabaseLocationError::Corrupt);
        }
        let mut target = self.read(name)?.ok_or(AppDatabaseLocationError::Corrupt)?;
        if !matches!(target.state, FileState::Writing) {
            return Err(AppDatabaseLocationError::Corrupt);
        }
        let path = self.location.database_path(name)?;
        if !path
            .symlink_metadata()
            .map_err(PlatformError::from)?
            .is_file()
        {
            return Err(AppDatabaseLocationError::Corrupt);
        }
        target.state = FileState::Activating {
            previous: previous.clone(),
        };
        self.write(name, &target)?;
        if let Some(mut old) = self.read(&previous)? {
            if !matches!(old.state, FileState::Active) {
                return Err(AppDatabaseLocationError::Corrupt);
            }
            old.kept_hashes = self.capture(&previous)?;
            old.kept_at = Some(at);
            old.state = FileState::Keeping { next: name.into() };
            self.write(&previous, &old)?;
        }
        Ok(())
    }

    pub(crate) fn activate_file(
        &self,
        name: &str,
        at: TimestampMillis,
    ) -> Result<(), AppDatabaseLocationError> {
        self.prepare_cutover(name, at)?;
        self.location.activate(name)?;
        self.recover()
    }

    pub fn inventory(&self, open: &Path) -> Result<Vec<AppDatabaseFile>, AppDatabaseLocationError> {
        let active = self.location.active_path()?;
        self.names()?
            .into_iter()
            .map(|name| {
                let path = self.location.database_path(&name)?;
                let metadata = self.read(&name)?.ok_or(AppDatabaseLocationError::Corrupt)?;
                let active = path == active;
                let deletable = !active
                    && path != open
                    && matches!(metadata.state, FileState::Kept | FileState::Abandoned)
                    && Database::try_reserve_file_deletion(&path)
                        .map_err(|_| AppDatabaseLocationError::Storage)?
                        .is_some();
                Ok(AppDatabaseFile {
                    file: name,
                    kind: metadata.kind,
                    created_at: metadata.created_at,
                    size: path.metadata().map_err(PlatformError::from)?.len(),
                    active,
                    deletable,
                })
            })
            .collect()
    }

    pub(super) fn is_complete(&self, name: &str) -> Result<bool, AppDatabaseLocationError> {
        Ok(self
            .read(name)?
            .is_some_and(|metadata| matches!(metadata.state, FileState::Active | FileState::Kept)))
    }

    pub fn kept_media_hashes(&self) -> Result<BTreeSet<ContentHash>, AppDatabaseLocationError> {
        let mut hashes = BTreeSet::new();
        for name in self.names()? {
            let metadata = self.read(&name)?.ok_or(AppDatabaseLocationError::Corrupt)?;
            if matches!(metadata.state, FileState::Kept) {
                hashes.extend(metadata.kept_hashes);
            }
        }
        Ok(hashes)
    }
}
