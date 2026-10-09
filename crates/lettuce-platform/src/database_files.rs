use cap_primitives::fs::FollowSymlinks;
use cap_std::fs::{Dir, OpenOptions};
use std::{fs::File, sync::Arc};

use crate::{ParentSyncStatus, PlatformError};

#[derive(Debug, Clone)]
pub struct DatabaseFiles {
    pub(crate) directory: Arc<Dir>,
    pub(crate) persistent: Arc<Dir>,
}

fn checked_name(name: &str) -> Result<(), PlatformError> {
    let stem = name
        .strip_suffix(".sqlite3")
        .ok_or(PlatformError::InvalidKey)?;
    if stem.is_empty()
        || stem.len() > 64
        || !stem
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(PlatformError::InvalidKey);
    }
    Ok(())
}

impl DatabaseFiles {
    pub fn contains_file(&self, target: &File) -> Result<bool, PlatformError> {
        let target = same_file::Handle::from_file(target.try_clone().map_err(PlatformError::from)?)
            .map_err(PlatformError::from)?;
        for directory in [&self.directory, &self.persistent] {
            for entry in directory.entries().map_err(PlatformError::from)? {
                let entry = entry.map_err(PlatformError::from)?;
                let name = entry.file_name();
                if !entry.file_type().map_err(PlatformError::from)?.is_file() {
                    continue;
                }
                let mut options = OpenOptions::new();
                options.read(true);
                options._cap_fs_ext_follow(FollowSymlinks::No);
                let file = directory
                    .open_with(&name, &options)
                    .map_err(crate::authority::map_symlink_error)?
                    .into_std();
                if same_file::Handle::from_file(file).map_err(PlatformError::from)? == target {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn remove_database_file(
        &self,
        name: &str,
        expected: &File,
    ) -> Result<ParentSyncStatus, PlatformError> {
        checked_name(name)?;
        let mut options = OpenOptions::new();
        options.read(true);
        options._cap_fs_ext_follow(FollowSymlinks::No);
        let opened = self
            .directory
            .open_with(name, &options)
            .map_err(crate::authority::map_symlink_error)?
            .into_std();
        if !opened.metadata().map_err(PlatformError::from)?.is_file() {
            return Err(PlatformError::Denied);
        }
        let actual = same_file::Handle::from_file(opened).map_err(PlatformError::from)?;
        let expected =
            same_file::Handle::from_file(expected.try_clone().map_err(PlatformError::from)?)
                .map_err(PlatformError::from)?;
        if actual != expected {
            return Err(PlatformError::Conflict);
        }
        self.directory
            .remove_file(name)
            .map_err(PlatformError::from)?;
        Ok(crate::managed::sync_directory(&self.directory))
    }

    pub fn validate_database_sidecars(&self, name: &str) -> Result<(), PlatformError> {
        self.sidecars(name).map(|_| ())
    }

    fn sidecars(&self, name: &str) -> Result<Vec<String>, PlatformError> {
        checked_name(name)?;
        let mut existing = Vec::new();
        for suffix in ["-wal", "-shm"] {
            let sidecar = format!("{name}{suffix}");
            match self.directory.symlink_metadata(&sidecar) {
                Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                    existing.push(sidecar)
                }
                Ok(_) => return Err(PlatformError::SymlinkEscape),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(PlatformError::from(error)),
            }
        }
        Ok(existing)
    }

    pub fn remove_database_sidecars(&self, name: &str) -> Result<ParentSyncStatus, PlatformError> {
        for sidecar in self.sidecars(name)? {
            self.directory
                .remove_file(sidecar)
                .map_err(PlatformError::from)?;
        }
        Ok(crate::managed::sync_directory(&self.directory))
    }
}
