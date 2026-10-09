use fs4::fs_std::FileExt;
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

use crate::{Database, DatabaseError};

pub(crate) struct DatabaseFileUse(File);

#[derive(Debug)]
pub struct DatabaseFileDeletionPermit(File);

fn open_file(path: &Path, create: bool) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(create);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    }
    Ok(file)
}

impl DatabaseFileUse {
    pub(crate) fn open(path: &Path) -> Result<Self, DatabaseError> {
        let file = open_file(path, true)?;
        FileExt::lock_shared(&file)?;
        Ok(Self(file))
    }
}

impl Database {
    pub fn try_reserve_file_deletion(
        path: &Path,
    ) -> Result<Option<DatabaseFileDeletionPermit>, DatabaseError> {
        let file = open_file(path, false)?;
        Ok(FileExt::try_lock_exclusive(&file)?.then_some(DatabaseFileDeletionPermit(file)))
    }
}

impl Drop for DatabaseFileUse {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

impl DatabaseFileDeletionPermit {
    pub fn file(&self) -> &File {
        &self.0
    }
}

impl Drop for DatabaseFileDeletionPermit {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}
