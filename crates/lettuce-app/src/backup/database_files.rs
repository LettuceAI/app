use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

use lettuce_platform::{ManagedFileLock, ParentSyncStatus};
use lettuce_types::ContentHash;
use serde::{Deserialize, Serialize};

use super::*;

const LIFECYCLE_KEY: &str = "database-file-lifecycle.lock";
const RESET_MOVE_KEY: &str = "database-reset-move.json";

#[derive(Debug, Clone, Copy)]
pub(super) enum ResetCutoverStage {
    Prepared,
    Closed,
    Intent,
    MainMoved,
    SidecarsMoved,
    Switched,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResetMoveIntent {
    version: u32,
    source: String,
    target: String,
    kept: String,
    metadata: FileMetadata,
    parts: Vec<String>,
}

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
    Deleting {
        request_id: lettuce_types::RequestId,
    },
    Deleted,
    Active,
    Kept,
    Activating {
        previous: String,
    },
    Keeping {
        next: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileMetadata {
    version: u32,
    kind: DatabaseFileKind,
    created_at: Option<TimestampMillis>,
    kept_at: Option<TimestampMillis>,
    kept_hashes: BTreeSet<ContentHash>,
    state: FileState,
}

#[derive(Debug, Clone)]
pub struct AppDatabaseFile {
    pub file: String,
    pub kind: DatabaseFileKind,
    pub created_at: Option<TimestampMillis>,
    pub modified_at: TimestampMillis,
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
    fn timestamp(time: SystemTime) -> Result<TimestampMillis, AppDatabaseLocationError> {
        let millis = time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AppDatabaseLocationError::Corrupt)?
            .as_millis();
        Ok(TimestampMillis::new(
            i64::try_from(millis).map_err(|_| AppDatabaseLocationError::Corrupt)?,
        ))
    }

    fn creation_time(
        created: std::io::Result<SystemTime>,
    ) -> Result<Option<TimestampMillis>, AppDatabaseLocationError> {
        match created {
            Ok(created) => Self::timestamp(created).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::Unsupported => Ok(None),
            Err(error) => Err(PlatformError::from(error).into()),
        }
    }

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
                    || metadata.created_at.is_none()
                        && matches!(
                            metadata.kind,
                            DatabaseFileKind::Restore
                                | DatabaseFileKind::LegacyRestore
                                | DatabaseFileKind::Reset
                        )
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
        self.recover_with_creation(std::fs::Metadata::created)
    }

    pub(crate) fn recover_reset_preparation(&self) -> Result<(), AppDatabaseLocationError> {
        self.recover()
    }

    pub(crate) fn reset_committed(&self, kept: &str) -> Result<bool, AppDatabaseLocationError> {
        Ok(self.read(kept)?.is_some_and(|metadata| {
            metadata.kept_at.is_some()
                && matches!(
                    metadata.state,
                    FileState::Kept | FileState::Deleting { .. } | FileState::Deleted
                )
        }))
    }

    fn recover_with_creation(
        &self,
        created: impl Fn(&std::fs::Metadata) -> std::io::Result<SystemTime>,
    ) -> Result<(), AppDatabaseLocationError> {
        self.recover_reset_move()?;
        let active = self.active_name()?;
        for name in self.names()? {
            let fence = Database::lock_file_writes(&self.location.database_path(&name)?)
                .map_err(|_| AppDatabaseLocationError::Storage)?;
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
                    let attributes = path.metadata().map_err(PlatformError::from)?;
                    let created_at = Self::creation_time(created(&attributes))?;
                    FileMetadata {
                        version: 1,
                        kind: if name == active {
                            DatabaseFileKind::Initial
                        } else {
                            DatabaseFileKind::Existing
                        },
                        created_at,
                        kept_at: if name != active {
                            Some(Self::timestamp(SystemTime::now())?)
                        } else {
                            None
                        },
                        kept_hashes: if name == active {
                            BTreeSet::new()
                        } else {
                            fence
                                .set_fenced(true)
                                .map_err(|_| AppDatabaseLocationError::Storage)?;
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
            let writable = matches!(metadata.state, FileState::Active)
                || name == active
                    && metadata.kind == DatabaseFileKind::Initial
                    && matches!(metadata.state, FileState::Writing | FileState::Abandoned);
            fence
                .set_fenced(!writable)
                .map_err(|_| AppDatabaseLocationError::Storage)?;
            if needs_write {
                self.write(&name, &metadata)?;
            }
        }
        Ok(())
    }

    fn write_reset_move(
        &self,
        intent: Option<&ResetMoveIntent>,
    ) -> Result<(), AppDatabaseLocationError> {
        let bytes = serde_json::to_vec(&intent).map_err(|_| AppDatabaseLocationError::Corrupt)?;
        let receipt = self.location.files.write_atomic(
            &self.location.write,
            ObjectKey::single(RESET_MOVE_KEY)?,
            &bytes,
        )?;
        if receipt.parent_sync == ParentSyncStatus::Failed {
            return Err(AppDatabaseLocationError::Storage);
        }
        Ok(())
    }

    fn fence_reset_name(&self, name: &str) -> Result<(), AppDatabaseLocationError> {
        self.location.database_path(name)?;
        let key = ObjectKey::from_segments([DATABASE_DIRECTORY, &format!("{name}.write-fenced")])?;
        match self.location.files.metadata(&self.location.read, &key) {
            Ok(metadata)
                if metadata.kind == lettuce_platform::ObjectKind::File && metadata.len == 0 =>
            {
                return Ok(());
            }
            Ok(_) => return Err(AppDatabaseLocationError::Corrupt),
            Err(PlatformError::NotFound) => {}
            Err(error) => return Err(error.into()),
        }
        let mut staged = self.location.files.stage_new(&self.location.write, key)?;
        use std::io::Write;
        staged.flush().map_err(PlatformError::from)?;
        if staged.commit()?.parent_sync == ParentSyncStatus::Failed {
            return Err(AppDatabaseLocationError::Storage);
        }
        Ok(())
    }

    fn move_reset_part(
        &self,
        from: &str,
        to: &str,
        suffix: &str,
    ) -> Result<(), AppDatabaseLocationError> {
        let expected = match self
            .location
            .directory
            .open_database_component(from, suffix)
        {
            Ok(file) => file,
            Err(PlatformError::NotFound) => self
                .location
                .directory
                .open_database_component(to, suffix)?,
            Err(error) => return Err(error.into()),
        };
        if self
            .location
            .directory
            .move_database_component(from, to, suffix, &expected)?
            == ParentSyncStatus::Failed
        {
            return Err(AppDatabaseLocationError::Storage);
        }
        Ok(())
    }

    fn recover_reset_move(&self) -> Result<(), AppDatabaseLocationError> {
        let bytes = match self
            .location
            .files
            .read(&self.location.read, &ObjectKey::single(RESET_MOVE_KEY)?)
        {
            Ok(bytes) => bytes,
            Err(PlatformError::NotFound) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let Some(intent): Option<ResetMoveIntent> =
            serde_json::from_slice(&bytes).map_err(|_| AppDatabaseLocationError::Corrupt)?
        else {
            return Ok(());
        };
        for name in [&intent.source, &intent.target, &intent.kept] {
            self.location.database_path(name)?;
        }
        if intent.version != 1
            || intent.source == intent.target
            || intent.source == intent.kept
            || intent.target == intent.kept
            || intent.parts.first().map(String::as_str) != Some("")
            || intent
                .parts
                .iter()
                .any(|part| !matches!(part.as_str(), "" | "-wal" | "-shm"))
            || intent.parts.iter().collect::<BTreeSet<_>>().len() != intent.parts.len()
        {
            return Err(AppDatabaseLocationError::Corrupt);
        }
        let active = self.active_name()?;
        if active != intent.source && active != intent.target {
            return Err(AppDatabaseLocationError::Corrupt);
        }
        let committed = active == intent.target;
        if committed {
            self.fence_reset_name(&intent.kept)?;
        }
        let (from, to) = if committed {
            (&intent.source, &intent.kept)
        } else {
            (&intent.kept, &intent.source)
        };
        for part in &intent.parts {
            self.move_reset_part(from, to, part)?;
        }
        let mut original = intent.metadata.clone();
        let mut kept = intent.metadata;
        if committed {
            original.state = FileState::Deleted;
            kept.state = FileState::Kept;
            self.write(&intent.kept, &kept)?;
        } else {
            original.state = FileState::Active;
            original.kept_at = None;
            original.kept_hashes.clear();
            kept.state = FileState::Deleted;
            kept.kept_at = None;
            kept.kept_hashes.clear();
            self.write(&intent.kept, &kept)?;
        }
        self.write(&intent.source, &original)?;
        self.write_reset_move(None)
    }

    pub(crate) fn reset_cutover(
        &self,
        target: &str,
        kept: &str,
        at: TimestampMillis,
        close: impl FnOnce() -> Result<(), AppDatabaseLocationError>,
    ) -> Result<(), AppDatabaseLocationError> {
        self.reset_cutover_with_checkpoint(target, kept, at, close, |_| {})
    }

    pub(super) fn reset_cutover_with_checkpoint(
        &self,
        target: &str,
        kept: &str,
        at: TimestampMillis,
        close: impl FnOnce() -> Result<(), AppDatabaseLocationError>,
        checkpoint: impl Fn(ResetCutoverStage),
    ) -> Result<(), AppDatabaseLocationError> {
        let result = (|| {
            let source = self.active_name()?;
            if source == kept
                || target == kept
                || self.read(kept)?.is_some()
                || self
                    .location
                    .database_path(kept)?
                    .try_exists()
                    .map_err(PlatformError::from)?
            {
                return Err(AppDatabaseLocationError::Exists);
            }
            self.prepare_cutover(target, at)?;
            checkpoint(ResetCutoverStage::Prepared);
            close()?;
            checkpoint(ResetCutoverStage::Closed);
            let permit =
                Database::try_reserve_file_deletion(&self.location.database_path(&source)?)
                    .map_err(|_| AppDatabaseLocationError::Storage)?
                    .ok_or(AppDatabaseLocationError::InUse)?;
            let mut parts = vec![String::new()];
            for suffix in ["-wal", "-shm"] {
                match self
                    .location
                    .directory
                    .open_database_component(&source, suffix)
                {
                    Ok(_) => parts.push(suffix.into()),
                    Err(PlatformError::NotFound) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            let intent = ResetMoveIntent {
                version: 1,
                source: source.clone(),
                target: target.into(),
                kept: kept.into(),
                metadata: self
                    .read(&source)?
                    .ok_or(AppDatabaseLocationError::Corrupt)?,
                parts,
            };
            self.write_reset_move(Some(&intent))?;
            self.write(kept, &intent.metadata)?;
            self.fence_reset_name(kept)?;
            checkpoint(ResetCutoverStage::Intent);
            self.move_reset_part(&source, kept, "")?;
            checkpoint(ResetCutoverStage::MainMoved);
            for part in intent.parts.iter().skip(1) {
                self.move_reset_part(&source, kept, part)?;
            }
            checkpoint(ResetCutoverStage::SidecarsMoved);
            self.location.activate(target)?;
            drop(permit);
            checkpoint(ResetCutoverStage::Switched);
            Ok(())
        })();
        self.recover()?;
        result
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
                created_at: Some(created_at),
                kept_at: None,
                kept_hashes: BTreeSet::new(),
                state: FileState::Writing,
            },
        )?;
        Ok(path)
    }

    pub(crate) fn prepare_open(
        &self,
        at: TimestampMillis,
    ) -> Result<PathBuf, AppDatabaseLocationError> {
        let path = self.location.active_path()?;
        if path.try_exists().map_err(PlatformError::from)? {
            return Ok(path);
        }
        match self.location.files.read(
            &self.location.read,
            &ObjectKey::single(ACTIVE_DATABASE_KEY)?,
        ) {
            Err(PlatformError::NotFound) => {}
            Ok(_) => return Err(AppDatabaseLocationError::NotFound),
            Err(error) => return Err(error.into()),
        }
        match self.read(INITIAL_DATABASE_NAME)? {
            None => self.begin_file(INITIAL_DATABASE_NAME, DatabaseFileKind::Initial, at),
            Some(metadata)
                if metadata.kind == DatabaseFileKind::Initial
                    && matches!(metadata.state, FileState::Writing | FileState::Abandoned) =>
            {
                Ok(path)
            }
            Some(_) => Err(AppDatabaseLocationError::NotFound),
        }
    }

    pub(crate) fn complete_open(&self) -> Result<(), AppDatabaseLocationError> {
        let name = self.active_name()?;
        let mut metadata = self.read(&name)?.ok_or(AppDatabaseLocationError::Corrupt)?;
        if matches!(metadata.state, FileState::Active) {
            return Ok(());
        }
        if metadata.kind != DatabaseFileKind::Initial
            || !matches!(metadata.state, FileState::Writing | FileState::Abandoned)
        {
            return Err(AppDatabaseLocationError::Corrupt);
        }
        metadata.state = FileState::Active;
        self.write(&name, &metadata)
    }

    pub(super) fn prepare_cutover(
        &self,
        name: &str,
        at: TimestampMillis,
    ) -> Result<(), AppDatabaseLocationError> {
        self.prepare_cutover_with_checkpoint(name, at, |_| {})
    }

    pub(super) fn prepare_cutover_with_checkpoint(
        &self,
        name: &str,
        at: TimestampMillis,
        checkpoint: impl Fn(FileKeepStage),
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
        checkpoint(FileKeepStage::TargetIntent);
        if let Some(mut old) = self.read(&previous)? {
            if !matches!(old.state, FileState::Active) {
                return Err(AppDatabaseLocationError::Corrupt);
            }
            let fence = Database::lock_file_writes(&self.location.database_path(&previous)?)
                .map_err(|_| AppDatabaseLocationError::Storage)?;
            old.kept_at = Some(at);
            old.state = FileState::Keeping { next: name.into() };
            self.write(&previous, &old)?;
            checkpoint(FileKeepStage::KeepIntent);
            fence
                .set_fenced(true)
                .map_err(|_| AppDatabaseLocationError::Storage)?;
            checkpoint(FileKeepStage::Frozen);
            old.kept_hashes = self.capture(&previous)?;
            self.write(&previous, &old)?;
            checkpoint(FileKeepStage::KeptSet);
        }
        Ok(())
    }

    pub(crate) fn activate_file(
        &self,
        name: &str,
        at: TimestampMillis,
    ) -> Result<(), AppDatabaseLocationError> {
        self.activate_file_with_switch(name, at, || self.location.activate(name))
    }

    fn activate_file_with_switch(
        &self,
        name: &str,
        at: TimestampMillis,
        switch: impl FnOnce() -> Result<(), AppDatabaseLocationError>,
    ) -> Result<(), AppDatabaseLocationError> {
        if let Err(error) = self.prepare_cutover(name, at).and_then(|()| switch()) {
            self.recover()?;
            return Err(error);
        }
        self.recover()
    }

    pub(crate) fn protects_export_location(
        &self,
        uri: &str,
    ) -> Result<bool, AppDatabaseLocationError> {
        let path = if uri.contains("://") {
            let uri = url::Url::parse(uri).map_err(|_| AppDatabaseLocationError::Corrupt)?;
            if uri.scheme() != "file" {
                return Ok(false);
            }
            uri.to_file_path()
                .map_err(|_| AppDatabaseLocationError::Corrupt)?
        } else {
            PathBuf::from(uri)
        };
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = parent.canonicalize().map_err(PlatformError::from)?;
        let private = self
            .location
            .private_persistent
            .canonicalize()
            .map_err(PlatformError::from)?;
        Ok(parent == private || parent.starts_with(private.join(DATABASE_DIRECTORY)))
    }

    pub(crate) fn protects_export_target(
        &self,
        target: &std::fs::File,
    ) -> Result<bool, AppDatabaseLocationError> {
        self.location
            .directory
            .contains_file(target)
            .map_err(Into::into)
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
                let attributes = path.metadata().map_err(PlatformError::from)?;
                Ok(AppDatabaseFile {
                    file: name,
                    kind: metadata.kind,
                    created_at: metadata.created_at,
                    modified_at: Self::timestamp(
                        attributes.modified().map_err(PlatformError::from)?,
                    )?,
                    size: attributes.len(),
                    active,
                    deletable,
                })
            })
            .collect()
    }

    pub(crate) fn is_complete(&self, name: &str) -> Result<bool, AppDatabaseLocationError> {
        Ok(self
            .read(name)?
            .is_some_and(|metadata| matches!(metadata.state, FileState::Active | FileState::Kept)))
    }

    pub fn kept_media_hashes(&self) -> Result<BTreeSet<ContentHash>, AppDatabaseLocationError> {
        let mut hashes = BTreeSet::new();
        for name in self.names()? {
            let metadata = self.read(&name)?.ok_or(AppDatabaseLocationError::Corrupt)?;
            if matches!(metadata.state, FileState::Kept | FileState::Deleting { .. }) {
                hashes.extend(metadata.kept_hashes);
            }
        }
        Ok(hashes)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteReceipt {
    version: u32,
    file: String,
    complete: bool,
}

impl DatabaseFileLifecycle {
    fn delete_receipt_key(
        key: lettuce_types::RequestId,
    ) -> Result<ObjectKey, AppDatabaseLocationError> {
        Ok(ObjectKey::single(format!(
            "database-file-delete-{key}.json"
        ))?)
    }

    fn write_delete_receipt(
        &self,
        key: lettuce_types::RequestId,
        receipt: &DeleteReceipt,
    ) -> Result<(), AppDatabaseLocationError> {
        let bytes = serde_json::to_vec(receipt).map_err(|_| AppDatabaseLocationError::Corrupt)?;
        let committed = self.location.files.write_atomic(
            &self.location.write,
            Self::delete_receipt_key(key)?,
            &bytes,
        )?;
        if committed.parent_sync == ParentSyncStatus::Failed {
            return Err(AppDatabaseLocationError::Storage);
        }
        Ok(())
    }

    pub(crate) fn delete_file(
        &self,
        name: &str,
        open: &Path,
        key: lettuce_types::RequestId,
    ) -> Result<(), AppDatabaseLocationError> {
        self.delete_file_with_checkpoint(name, open, key, |_| {})
    }

    pub(super) fn delete_file_with_checkpoint(
        &self,
        name: &str,
        open: &Path,
        key: lettuce_types::RequestId,
        checkpoint: impl Fn(FileDeleteStage),
    ) -> Result<(), AppDatabaseLocationError> {
        let path = self.location.database_path(name)?;
        let receipt = match self
            .location
            .files
            .read(&self.location.read, &Self::delete_receipt_key(key)?)
        {
            Ok(bytes) => {
                let receipt: DeleteReceipt = serde_json::from_slice(&bytes)
                    .map_err(|_| AppDatabaseLocationError::Corrupt)?;
                if receipt.version != 1 {
                    return Err(AppDatabaseLocationError::Corrupt);
                }
                if receipt.file != name {
                    return Err(AppDatabaseLocationError::Conflict);
                }
                if receipt.complete {
                    return Ok(());
                }
                Some(receipt)
            }
            Err(PlatformError::NotFound) => None,
            Err(error) => return Err(error.into()),
        };
        if path == open || path == self.location.active_path()? {
            return Err(AppDatabaseLocationError::InUse);
        }
        let mut metadata = self.read(name)?.ok_or(AppDatabaseLocationError::NotFound)?;
        match metadata.state {
            FileState::Kept | FileState::Abandoned => {}
            FileState::Deleting { request_id } if request_id == key => {}
            FileState::Deleted if receipt.is_some() => {}
            _ => return Err(AppDatabaseLocationError::InUse),
        }
        let permit = if path.try_exists().map_err(PlatformError::from)? {
            Some(
                Database::try_reserve_file_deletion(&path)
                    .map_err(|_| AppDatabaseLocationError::Storage)?
                    .ok_or(AppDatabaseLocationError::InUse)?,
            )
        } else if receipt.is_some() {
            None
        } else {
            return Err(AppDatabaseLocationError::NotFound);
        };
        self.location.directory.validate_database_sidecars(name)?;
        if receipt.is_none() {
            self.write_delete_receipt(
                key,
                &DeleteReceipt {
                    version: 1,
                    file: name.into(),
                    complete: false,
                },
            )?;
            metadata.state = FileState::Deleting { request_id: key };
            self.write(name, &metadata)?;
        }
        checkpoint(FileDeleteStage::Intent);
        if let Some(permit) = &permit {
            let synced = self
                .location
                .directory
                .remove_database_file(name, permit.file())?;
            if synced == ParentSyncStatus::Failed {
                return Err(AppDatabaseLocationError::Storage);
            }
        }
        checkpoint(FileDeleteStage::DatabaseRemoved);
        if self.location.directory.remove_database_sidecars(name)? == ParentSyncStatus::Failed {
            return Err(AppDatabaseLocationError::Storage);
        }
        checkpoint(FileDeleteStage::SidecarsRemoved);
        metadata.state = FileState::Deleted;
        metadata.kept_hashes.clear();
        self.write(name, &metadata)?;
        checkpoint(FileDeleteStage::MetadataRemoved);
        self.write_delete_receipt(
            key,
            &DeleteReceipt {
                version: 1,
                file: name.into(),
                complete: true,
            },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FileKeepStage {
    TargetIntent,
    KeepIntent,
    Frozen,
    KeptSet,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FileDeleteStage {
    Intent,
    DatabaseRemoved,
    SidecarsRemoved,
    MetadataRemoved,
}

#[cfg(test)]
mod timestamp_tests {
    use super::*;

    #[test]
    fn failed_pointer_switch_restores_old_writes_before_returning() {
        let root = std::env::temp_dir().join(format!(
            "lettuce-cutover-failed-switch-{}",
            OperationId::new()
        ));
        let location = unopened_location(&root);
        let active = location.active_path().expect("active");
        let database = Database::open(&active).expect("old database");
        let guard = location.try_file_lifecycle().expect("lifecycle");
        let name = format!("{}.sqlite3", OperationId::new());
        let new_path = guard
            .begin_file(&name, DatabaseFileKind::Reset, TimestampMillis::new(20))
            .expect("new intent");
        drop(Database::open(&new_path).expect("new database"));
        assert_eq!(
            guard.activate_file_with_switch(&name, TimestampMillis::new(30), || {
                Err(AppDatabaseLocationError::Storage)
            }),
            Err(AppDatabaseLocationError::Storage)
        );
        assert_eq!(location.active_path().expect("pointer"), active);
        assert!(
            !database
                .is_file_write_fenced()
                .expect("old writes recovered")
        );
        drop(Database::open(&active).expect("old database opens writable"));
        let files = guard.inventory(&active).expect("inventory");
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|file| file.active && !file.deletable));
        assert!(files.iter().any(|file| file.file == name && !file.active));
        assert!(guard.kept_media_hashes().expect("kept hashes").is_empty());
        drop(guard);
        drop(database);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    fn unopened_location(root: &Path) -> AppDatabaseLocation {
        let authority = FilesystemAuthority::new(
            &lettuce_platform::DirectorySnapshot::new(root).expect("snapshot"),
        )
        .expect("authority");
        AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority).expect("location")
    }

    fn unrecovered_guard(location: &AppDatabaseLocation) -> DatabaseFileLifecycle {
        DatabaseFileLifecycle {
            location: location.clone(),
            _lock: location
                .files
                .lock_file(
                    &location.write,
                    &ObjectKey::single(LIFECYCLE_KEY).expect("key"),
                    false,
                )
                .expect("lock")
                .expect("available lock"),
        }
    }

    #[test]
    fn unsupported_creation_time_bootstraps_without_substituting_a_date() {
        let root =
            std::env::temp_dir().join(format!("lettuce-unknown-created-{}", OperationId::new()));
        let location = unopened_location(&root);
        let active = location.active_path().expect("active");
        drop(Database::open(&active).expect("database"));
        let before = std::fs::read(&active).expect("database bytes");
        let measured = DatabaseFileLifecycle::timestamp(
            active
                .metadata()
                .expect("metadata")
                .modified()
                .expect("modified time"),
        )
        .expect("timestamp");
        let guard = unrecovered_guard(&location);
        guard
            .recover_with_creation(|_| Err(std::io::ErrorKind::Unsupported.into()))
            .expect("creation time is optional for existing files");
        let file = guard.inventory(&active).expect("inventory").remove(0);
        assert_eq!(file.created_at, None);
        assert_eq!(file.modified_at, measured);
        assert_eq!(std::fs::read(&active).expect("database bytes"), before);
        let name = format!("{}.sqlite3", OperationId::new());
        let path = guard
            .begin_file(&name, DatabaseFileKind::Reset, TimestampMillis::new(20))
            .expect("new file");
        drop(Database::open(&path).expect("new database"));
        guard
            .activate_file(&name, TimestampMillis::new(30))
            .expect("cutover");
        drop(guard);
        let guard = location.try_file_lifecycle().expect("reopen");
        let files = guard.inventory(&path).expect("inventory");
        assert!(
            files
                .iter()
                .any(|file| file.file == "lettuce.sqlite3" && file.created_at.is_none())
        );
        assert!(
            files
                .iter()
                .any(|file| file.file == name && file.created_at == Some(TimestampMillis::new(20)))
        );
        drop(guard);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn failed_creation_time_read_is_not_converted_to_unknown() {
        let root =
            std::env::temp_dir().join(format!("lettuce-unreadable-created-{}", OperationId::new()));
        let location = unopened_location(&root);
        let active = location.active_path().expect("active");
        drop(Database::open(&active).expect("database"));
        let guard = unrecovered_guard(&location);
        assert!(
            guard
                .recover_with_creation(|_| Err(std::io::ErrorKind::PermissionDenied.into()))
                .is_err()
        );
        assert!(
            guard
                .read("lettuce.sqlite3")
                .expect("read metadata")
                .is_none()
        );
        drop(guard);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn first_database_creation_keeps_its_recorded_time_across_interruption() {
        for created in [false, true] {
            let root = std::env::temp_dir()
                .join(format!("lettuce-initial-created-{}", OperationId::new()));
            let location = unopened_location(&root);
            let child = std::process::Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "backup::backup_restore::database_files::timestamp_tests::initial_creation_crash_child",
                    "--nocapture",
                ])
                .env("LETTUCE_SLICE12_INITIAL_CRASH_ROOT", &root)
                .env("LETTUCE_SLICE12_INITIAL_CREATED", created.to_string())
                .status()
                .expect("child");
            assert_eq!(child.code(), Some(77));
            let path = location.active_path().expect("active");
            let guard = location.try_file_lifecycle().expect("recover");
            assert_eq!(
                guard
                    .prepare_open(TimestampMillis::new(40))
                    .expect("resume"),
                path
            );
            drop(Database::open(&path).expect("database"));
            guard.complete_open().expect("complete creation");
            let file = guard.inventory(&path).expect("inventory").remove(0);
            assert_eq!(file.created_at, Some(TimestampMillis::new(20)));
            assert!(file.active && !file.deletable);
            drop(guard);
            std::fs::remove_dir_all(root).expect("cleanup");
        }
    }

    #[test]
    fn initial_creation_crash_child() {
        let Some(root) = std::env::var_os("LETTUCE_SLICE12_INITIAL_CRASH_ROOT") else {
            return;
        };
        let location = unopened_location(Path::new(&root));
        let guard = location.try_file_lifecycle().expect("lock");
        let path = guard
            .prepare_open(TimestampMillis::new(20))
            .expect("initial creation");
        if std::env::var("LETTUCE_SLICE12_INITIAL_CREATED").expect("stage") == "true" {
            drop(Database::open(&path).expect("database"));
        }
        std::process::exit(77);
    }

    #[test]
    fn opening_a_missing_switched_database_does_not_create_an_empty_replacement() {
        let root =
            std::env::temp_dir().join(format!("lettuce-missing-active-{}", OperationId::new()));
        let location = unopened_location(&root);
        location.activate("missing.sqlite3").expect("pointer");
        let guard = location.try_file_lifecycle().expect("lock");
        assert!(matches!(
            guard.prepare_open(TimestampMillis::new(20)),
            Err(AppDatabaseLocationError::NotFound)
        ));
        assert!(!location.active_path().expect("active path").exists());
        drop(guard);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn pre_existing_file_metadata_accepts_an_unknown_creation_time() {
        let metadata: FileMetadata = serde_json::from_value(serde_json::json!({
            "version": 1,
            "kind": "existing",
            "created_at": null,
            "kept_at": 123,
            "kept_hashes": [],
            "state": {"state": "kept"}
        }))
        .expect("unknown filesystem creation time is valid");
        assert_eq!(
            serde_json::to_value(metadata).expect("metadata JSON")["created_at"],
            serde_json::Value::Null
        );
    }
}
