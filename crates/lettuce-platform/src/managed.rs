use std::{
    fmt,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{Arc, MutexGuard},
};

use cap_primitives::fs::{FollowSymlinks, stat};
use cap_std::fs::{Dir, File as CapFile, OpenOptions};

use crate::{
    authority::{AuthorityInner, RootHandle, map_symlink_error, open_dir_nofollow_from},
    directories::ManagedRoot,
    error::PlatformError,
    keys::ObjectKey,
    model::{
        DirectoryEntry, ObjectKind, ObjectMetadata, ReadCapability, ReadHandle, WriteCapability,
    },
};

pub(crate) const MAX_READ_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_RECOVERY_ARTIFACTS: usize = 256;
pub(crate) const MAX_RECOVERY_DEPTH: usize = 16;
pub(crate) const MAX_LIST_ENTRIES: usize = 1024;

/// Operational filesystem facade. It has no constructor or capability-minting
/// method; the composition authority must issue this alongside purpose caps.
#[derive(Clone)]
pub struct ManagedFiles {
    pub(crate) inner: Arc<AuthorityInner>,
}

impl ManagedFiles {
    pub(crate) fn from_authority(inner: Arc<AuthorityInner>) -> Self {
        Self { inner }
    }

    pub fn read(
        &self,
        capability: &ReadCapability,
        key: &ObjectKey,
    ) -> Result<Vec<u8>, PlatformError> {
        let file = self.open_read(capability, key)?;
        let mut bytes = Vec::new();
        file.take(MAX_READ_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(PlatformError::from)?;
        if bytes.len() as u64 > MAX_READ_BYTES {
            return Err(PlatformError::LimitExceeded);
        }
        Ok(bytes)
    }

    pub fn open_read(
        &self,
        capability: &ReadCapability,
        key: &ObjectKey,
    ) -> Result<ReadHandle, PlatformError> {
        let root = self.check_read(capability)?;
        let (parent, name) = resolve_parent(root, &key.segments, false)?;
        let file = open_file_nofollow(&parent, &name, true, false).map_err(map_symlink_error)?;
        Ok(ReadHandle {
            file: file.into_std(),
        })
    }

    pub fn metadata(
        &self,
        capability: &ReadCapability,
        key: &ObjectKey,
    ) -> Result<ObjectMetadata, PlatformError> {
        let root = self.check_read(capability)?;
        let (parent, name) = resolve_parent(root, &key.segments, false)?;
        let native = parent
            .try_clone()
            .map_err(PlatformError::from)?
            .into_std_file();
        let metadata = stat(&native, &name, FollowSymlinks::No).map_err(map_symlink_error)?;
        Ok(ObjectMetadata {
            kind: object_kind(&metadata),
            len: metadata.len(),
        })
    }

    pub fn list(
        &self,
        capability: &ReadCapability,
        prefix: Option<&ObjectKey>,
        limit: usize,
    ) -> Result<Vec<DirectoryEntry>, PlatformError> {
        let root = self.check_read(capability)?;
        if limit == 0 || limit > MAX_LIST_ENTRIES {
            return Err(PlatformError::LimitExceeded);
        }
        if capability.root == ManagedRoot::PrivatePersistent {
            return Err(PlatformError::Unsupported);
        }
        let directory = match prefix {
            Some(key) => open_directory(root, &key.segments, false)?,
            None => root.dir.try_clone().map_err(PlatformError::from)?,
        };
        let mut entries = Vec::new();
        let mut inspected = 0usize;
        for item in directory.read_dir(".").map_err(PlatformError::from)? {
            let item = item.map_err(PlatformError::from)?;
            inspected += 1;
            if entries.len() >= limit || inspected > MAX_LIST_ENTRIES {
                return Err(PlatformError::LimitExceeded);
            }
            let name = item
                .file_name()
                .into_string()
                .map_err(|_| PlatformError::InvalidKey)?;
            if name.chars().any(char::is_control) {
                continue;
            }
            // DirEntry::file_type is no-follow; a symlink is therefore Other
            // and cannot disclose the target's metadata.
            entries.push(DirectoryEntry {
                name,
                kind: object_kind_from_file_type(&item.file_type().map_err(PlatformError::from)?),
            });
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    pub(crate) fn check_read(
        &self,
        capability: &ReadCapability,
    ) -> Result<&RootHandle, PlatformError> {
        if !Arc::ptr_eq(&self.inner, &capability.inner) {
            return Err(PlatformError::WrongCapability);
        }
        self.inner
            .roots
            .get(&capability.root)
            .ok_or(PlatformError::InvalidRoot)
    }

    pub(crate) fn check_write(
        &self,
        capability: &WriteCapability,
    ) -> Result<&RootHandle, PlatformError> {
        if !Arc::ptr_eq(&self.inner, &capability.inner) {
            return Err(PlatformError::WrongCapability);
        }
        self.inner
            .roots
            .get(&capability.root)
            .ok_or(PlatformError::InvalidRoot)
    }

    pub(crate) fn mutation_guard(&self) -> Result<MutexGuard<'_, ()>, PlatformError> {
        self.inner
            .mutation_lock
            .lock()
            .map_err(|_| PlatformError::RecoveryNeeded)
    }
}

impl fmt::Debug for ManagedFiles {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ManagedFiles(<authority-bound>)")
    }
}

pub(crate) fn resolve_parent(
    root: &RootHandle,
    segments: &[String],
    create: bool,
) -> Result<(Dir, PathBuf), PlatformError> {
    let (parents, target) = segments.split_at(
        segments
            .len()
            .checked_sub(1)
            .ok_or(PlatformError::InvalidKey)?,
    );
    let mut current = root.dir.try_clone().map_err(PlatformError::from)?;
    for segment in parents {
        let name = Path::new(segment);
        match open_dir_nofollow_from(&current, name) {
            Ok(next) => current = next,
            Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
                match current.create_dir(name) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(PlatformError::from(error)),
                }
                current = open_dir_nofollow_from(&current, name).map_err(map_symlink_error)?;
            }
            Err(error) => return Err(map_symlink_error(error)),
        }
    }
    Ok((current, PathBuf::from(target[0].as_str())))
}

pub(crate) fn open_directory(
    root: &RootHandle,
    segments: &[String],
    create: bool,
) -> Result<Dir, PlatformError> {
    let mut current = root.dir.try_clone().map_err(PlatformError::from)?;
    for segment in segments {
        let name = Path::new(segment);
        match open_dir_nofollow_from(&current, name) {
            Ok(next) => current = next,
            Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
                match current.create_dir(name) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(PlatformError::from(error)),
                }
                current = open_dir_nofollow_from(&current, name).map_err(map_symlink_error)?;
            }
            Err(error) => return Err(map_symlink_error(error)),
        }
    }
    Ok(current)
}

pub(crate) fn open_file_nofollow(
    parent: &Dir,
    name: &Path,
    read: bool,
    create_new: bool,
) -> io::Result<CapFile> {
    let mut options = OpenOptions::new();
    options.read(read).write(!read).create_new(create_new);
    options._cap_fs_ext_follow(cap_primitives::fs::FollowSymlinks::No);
    parent.open_with(name, &options)
}

pub(crate) fn object_kind(metadata: &cap_std::fs::Metadata) -> ObjectKind {
    if metadata.is_file() {
        ObjectKind::File
    } else if metadata.is_dir() {
        ObjectKind::Directory
    } else {
        ObjectKind::Other
    }
}

pub(crate) fn object_kind_from_file_type(file_type: &cap_std::fs::FileType) -> ObjectKind {
    if file_type.is_file() {
        ObjectKind::File
    } else if file_type.is_dir() {
        ObjectKind::Directory
    } else {
        ObjectKind::Other
    }
}

pub(crate) fn target_parent_segments(key: &ObjectKey) -> Vec<String> {
    key.segments[..key.segments.len() - 1].to_vec()
}

pub(crate) fn sync_directory(directory: &Dir) -> crate::model::ParentSyncStatus {
    #[cfg(unix)]
    {
        if directory
            .try_clone()
            .and_then(|directory| directory.into_std_file().sync_all())
            .is_ok()
        {
            crate::model::ParentSyncStatus::Synced
        } else {
            crate::model::ParentSyncStatus::Failed
        }
    }
    #[cfg(not(unix))]
    {
        let _ = directory;
        crate::model::ParentSyncStatus::Unsupported
    }
}
