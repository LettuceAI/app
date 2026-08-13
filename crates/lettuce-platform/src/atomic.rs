use std::{
    fmt,
    io::{self, Write},
    path::Path,
    sync::Arc,
};

use cap_std::fs::File as CapFile;
use uuid::Uuid;

use crate::{
    error::PlatformError,
    keys::STAGE_PREFIX,
    managed::{
        ManagedFiles, open_file_nofollow, resolve_parent, sync_directory, target_parent_segments,
    },
    model::{Atomicity, CommitReceipt, StageCleanupStatus, WriteCapability, WriteMode},
};

impl ManagedFiles {
    pub fn stage(
        &self,
        capability: &WriteCapability,
        key: crate::ObjectKey,
    ) -> Result<StagedWrite, PlatformError> {
        self.stage_with_mode(capability, key, WriteMode::Replace, None)
    }

    pub fn stage_new(
        &self,
        capability: &WriteCapability,
        key: crate::ObjectKey,
    ) -> Result<StagedWrite, PlatformError> {
        self.stage_with_mode(capability, key, WriteMode::CreateNew, None)
    }

    pub fn stage_bounded(
        &self,
        capability: &WriteCapability,
        key: crate::ObjectKey,
        max_bytes: u64,
    ) -> Result<StagedWrite, PlatformError> {
        self.stage_with_mode(capability, key, WriteMode::Replace, Some(max_bytes))
    }

    pub fn write_atomic(
        &self,
        capability: &WriteCapability,
        key: crate::ObjectKey,
        bytes: &[u8],
    ) -> Result<CommitReceipt, PlatformError> {
        let mut staged = self.stage(capability, key)?;
        staged.write_all(bytes).map_err(|error| {
            if error.kind() == io::ErrorKind::InvalidInput {
                PlatformError::LimitExceeded
            } else {
                PlatformError::from(error)
            }
        })?;
        staged.commit()
    }

    fn stage_with_mode(
        &self,
        capability: &WriteCapability,
        key: crate::ObjectKey,
        mode: WriteMode,
        max_bytes: Option<u64>,
    ) -> Result<StagedWrite, PlatformError> {
        let root = self.check_write(capability)?;
        let (parent, _) = resolve_parent(root, &key.segments, true)?;
        let stage_parent = target_parent_segments(&key);
        let stage_name = format!("{STAGE_PREFIX}{}", Uuid::new_v4().simple());
        let file = open_file_nofollow(&parent, Path::new(&stage_name), false, true)
            .map_err(crate::authority::map_symlink_error)?;
        Ok(StagedWrite {
            inner: Arc::clone(&self.inner),
            root: capability.root,
            target: key,
            stage_parent,
            stage_name,
            file: Some(file),
            max_bytes,
            bytes_written: 0,
            mode,
            retain_stage: false,
        })
    }
}

/// A sibling stage file with bounded streaming writes. Dropping an
/// uncommitted writer removes only its own tool-owned stage file. Failed
/// commits retain their stage for non-destructive recovery.
pub struct StagedWrite {
    pub(crate) inner: Arc<crate::authority::AuthorityInner>,
    pub(crate) root: crate::directories::ManagedRoot,
    pub(crate) target: crate::ObjectKey,
    pub(crate) stage_parent: Vec<String>,
    pub(crate) stage_name: String,
    pub(crate) file: Option<CapFile>,
    pub(crate) max_bytes: Option<u64>,
    pub(crate) bytes_written: u64,
    pub(crate) mode: WriteMode,
    pub(crate) retain_stage: bool,
}

impl fmt::Debug for StagedWrite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StagedWrite")
            .field("bytes_written", &self.bytes_written)
            .field("mode", &self.mode)
            .finish()
    }
}

impl Write for StagedWrite {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if let Some(limit) = self.max_bytes {
            if self.bytes_written.saturating_add(buffer.len() as u64) > limit {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "write limit exceeded",
                ));
            }
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "writer is closed"))?;
        let written = file.write(buffer)?;
        self.bytes_written = self.bytes_written.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "writer is closed"))?
            .flush()
    }
}

impl StagedWrite {
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    pub fn commit(mut self) -> Result<CommitReceipt, PlatformError> {
        let file = self.file.take().ok_or(PlatformError::Io)?;
        file.sync_all().map_err(|error| {
            self.retain_stage = true;
            PlatformError::from(error)
        })?;
        drop(file);
        let result = commit_staged(
            &self.inner,
            self.root,
            &self.target,
            &self.stage_parent,
            &self.stage_name,
            self.mode,
            self.bytes_written,
        );
        match result {
            Ok(outcome) => {
                self.retain_stage = outcome.retain_stage;
                Ok(outcome.receipt)
            }
            Err(error) => {
                self.retain_stage = true;
                Err(error)
            }
        }
    }

    pub fn abort(mut self) -> Result<(), PlatformError> {
        self.file.take();
        remove_stage(&self.inner, self.root, &self.stage_parent, &self.stage_name)
    }
}

impl Drop for StagedWrite {
    fn drop(&mut self) {
        self.file.take();
        if !self.retain_stage {
            let _ = remove_stage(&self.inner, self.root, &self.stage_parent, &self.stage_name);
        }
    }
}

struct CommitOutcome {
    receipt: CommitReceipt,
    retain_stage: bool,
}

fn commit_staged(
    inner: &Arc<crate::authority::AuthorityInner>,
    root: crate::directories::ManagedRoot,
    target: &crate::ObjectKey,
    stage_parent_segments: &[String],
    stage_name: &str,
    mode: WriteMode,
    bytes_written: u64,
) -> Result<CommitOutcome, PlatformError> {
    let _guard = inner
        .mutation_lock
        .lock()
        .map_err(|_| PlatformError::RecoveryNeeded)?;
    let root_handle = inner.roots.get(&root).ok_or(PlatformError::InvalidRoot)?;
    let (parent, target_name) = resolve_parent(root_handle, &target.segments, false)?;
    let stage_parent = crate::managed::open_directory(root_handle, stage_parent_segments, false)?;
    if stage_parent
        .symlink_metadata(Path::new(stage_name))
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(PlatformError::SymlinkEscape);
    }

    let stage_cleanup = match mode {
        WriteMode::CreateNew => {
            // cap-std rename replaces an existing destination. Hard-linking a
            // synced sibling gives create-new no-replace semantics atomically;
            // filesystems that cannot hard-link report Unsupported.
            match stage_parent.hard_link(Path::new(stage_name), &parent, &target_name) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    return Err(PlatformError::Conflict);
                }
                Err(error) => return Err(map_create_new_error(error)),
            }
            classify_stage_cleanup(stage_parent.remove_file(Path::new(stage_name)))
        }
        WriteMode::Replace => {
            if parent
                .symlink_metadata(&target_name)
                .map(|metadata| metadata.file_type().is_symlink())
                .unwrap_or(false)
            {
                return Err(PlatformError::SymlinkEscape);
            }
            #[cfg(windows)]
            if parent.symlink_metadata(&target_name).is_ok() {
                return Err(PlatformError::ReplaceFailed);
            }
            stage_parent
                .rename(Path::new(stage_name), &parent, &target_name)
                .map_err(|error| {
                    if error.kind() == io::ErrorKind::AlreadyExists {
                        PlatformError::Conflict
                    } else {
                        PlatformError::ReplaceFailed
                    }
                })?;
            StageCleanupStatus::Cleaned
        }
    };
    let parent_sync = sync_directory(&parent);
    Ok(CommitOutcome {
        receipt: CommitReceipt {
            atomicity: Atomicity::Atomic,
            parent_sync,
            stage_cleanup,
            bytes_written,
        },
        retain_stage: stage_cleanup == StageCleanupStatus::RecoveryNeeded,
    })
}

pub(crate) fn classify_stage_cleanup(result: io::Result<()>) -> StageCleanupStatus {
    match result {
        Ok(()) => StageCleanupStatus::Cleaned,
        Err(error) if error.kind() == io::ErrorKind::NotFound => StageCleanupStatus::Cleaned,
        Err(_) => StageCleanupStatus::RecoveryNeeded,
    }
}

fn map_create_new_error(error: io::Error) -> PlatformError {
    match error.kind() {
        io::ErrorKind::Unsupported | io::ErrorKind::CrossesDevices => PlatformError::Unsupported,
        io::ErrorKind::PermissionDenied => PlatformError::Denied,
        _ => PlatformError::from(error),
    }
}

fn remove_stage(
    inner: &Arc<crate::authority::AuthorityInner>,
    root: crate::directories::ManagedRoot,
    parent_segments: &[String],
    stage_name: &str,
) -> Result<(), PlatformError> {
    if !crate::keys::is_owned_stage_name(stage_name) {
        return Err(PlatformError::InvalidKey);
    }
    let handle = inner.roots.get(&root).ok_or(PlatformError::InvalidRoot)?;
    let parent = crate::managed::open_directory(handle, parent_segments, false)?;
    match parent.remove_file(Path::new(stage_name)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(PlatformError::from(error)),
    }
}
