use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use cap_primitives::fs::FollowSymlinks;
use cap_std::{ambient_authority, fs::OpenOptions};

use crate::{ObjectKey, PlatformError};

#[derive(Debug)]
pub struct ConfinedInstallStore {
    root_path: PathBuf,
    root: cap_std::fs::Dir,
}

#[derive(Debug)]
pub enum InstallPreparation {
    Installed(InstalledFile),
    Resume(ResumableInstall),
}

#[derive(Debug)]
pub struct InstalledFile {
    path: PathBuf,
    file: cap_std::fs::File,
    len: u64,
}

#[derive(Debug)]
pub struct ResumableInstall {
    root_path: PathBuf,
    root: cap_std::fs::Dir,
    file: cap_std::fs::File,
    partial: ObjectKey,
    target: ObjectKey,
    offset: u64,
    max_bytes: u64,
}

impl ConfinedInstallStore {
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self, PlatformError> {
        let root_path = root_path.as_ref();
        if !root_path.is_absolute() {
            return Err(PlatformError::Denied);
        }
        match std::fs::symlink_metadata(root_path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(PlatformError::SymlinkEscape);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir_all(root_path).map_err(PlatformError::from)?;
            }
            Err(error) => return Err(PlatformError::from(error)),
        }
        let root_path = root_path.canonicalize().map_err(PlatformError::from)?;
        let root = cap_std::fs::Dir::open_ambient_dir(&root_path, ambient_authority())
            .map_err(PlatformError::from)?;
        Ok(Self { root_path, root })
    }

    pub fn prepare(
        &self,
        partial: ObjectKey,
        target: ObjectKey,
        max_bytes: u64,
    ) -> Result<InstallPreparation, PlatformError> {
        if max_bytes == 0 {
            return Err(PlatformError::LimitExceeded);
        }
        let target_path = path_for(&target);
        create_parent(&self.root, &target)?;
        match self.root.symlink_metadata(&target_path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(PlatformError::SymlinkEscape);
                }
                if metadata.len() > max_bytes {
                    return Err(PlatformError::LimitExceeded);
                }
                let file = self.root.open(&target_path).map_err(PlatformError::from)?;
                return Ok(InstallPreparation::Installed(InstalledFile {
                    path: self.root_path.join(target_path),
                    file,
                    len: metadata.len(),
                }));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(PlatformError::from(error)),
        }
        create_parent(&self.root, &partial)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        options._cap_fs_ext_follow(FollowSymlinks::No);
        let mut file = self
            .root
            .open_with(path_for(&partial), &options)
            .map_err(PlatformError::from)?;
        let offset = file.metadata().map_err(PlatformError::from)?.len();
        if offset > max_bytes {
            return Err(PlatformError::LimitExceeded);
        }
        file.seek(SeekFrom::End(0)).map_err(PlatformError::from)?;
        Ok(InstallPreparation::Resume(ResumableInstall {
            root_path: self.root_path.clone(),
            root: self.root.try_clone().map_err(PlatformError::from)?,
            file,
            partial,
            target,
            offset,
            max_bytes,
        }))
    }
}

impl InstalledFile {
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn rewind(&mut self) -> Result<(), PlatformError> {
        self.file
            .seek(SeekFrom::Start(0))
            .map(|_| ())
            .map_err(PlatformError::from)
    }

    #[must_use]
    pub fn native_path(&self) -> &Path {
        &self.path
    }
}

impl Read for InstalledFile {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buffer)
    }
}

impl ResumableInstall {
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub fn restart(&mut self) -> Result<(), PlatformError> {
        self.file.set_len(0).map_err(PlatformError::from)?;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(PlatformError::from)?;
        self.offset = 0;
        Ok(())
    }

    pub fn append(&mut self, bytes: &[u8]) -> Result<u64, PlatformError> {
        let next = self
            .offset
            .checked_add(u64::try_from(bytes.len()).map_err(|_| PlatformError::LimitExceeded)?)
            .ok_or(PlatformError::LimitExceeded)?;
        if next > self.max_bytes {
            return Err(PlatformError::LimitExceeded);
        }
        self.file.write_all(bytes).map_err(PlatformError::from)?;
        self.file.flush().map_err(PlatformError::from)?;
        self.offset = next;
        Ok(next)
    }

    pub fn rewind(&mut self) -> Result<(), PlatformError> {
        self.file
            .seek(SeekFrom::Start(0))
            .map(|_| ())
            .map_err(PlatformError::from)
    }

    pub fn sync(&self) -> Result<(), PlatformError> {
        self.file.sync_all().map_err(PlatformError::from)
    }

    pub fn commit(self) -> Result<PathBuf, PlatformError> {
        self.file.sync_all().map_err(PlatformError::from)?;
        drop(self.file);
        let source = path_for(&self.partial);
        let target = path_for(&self.target);
        self.root
            .rename(&source, &self.root, &target)
            .map_err(PlatformError::from)?;
        Ok(self.root_path.join(target))
    }
}

impl Read for ResumableInstall {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buffer)
    }
}

fn create_parent(root: &cap_std::fs::Dir, key: &ObjectKey) -> Result<(), PlatformError> {
    let parent = &key.segments[..key.segments.len() - 1];
    let mut path = PathBuf::new();
    for segment in parent {
        path.push(segment);
        match root.create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(PlatformError::from(error)),
        }
        if root
            .symlink_metadata(&path)
            .map_err(PlatformError::from)?
            .file_type()
            .is_symlink()
        {
            return Err(PlatformError::SymlinkEscape);
        }
    }
    Ok(())
}

fn path_for(key: &ObjectKey) -> PathBuf {
    key.segments.iter().collect()
}
