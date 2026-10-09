use fs4::fs_std::FileExt;
use std::{
    fs::{File, OpenOptions},
    path::Path,
    sync::{Mutex, atomic::Ordering},
};

use crate::{Database, DatabaseError};

pub(crate) struct DatabaseFileUse(Mutex<Option<File>>);

#[derive(Debug)]
pub struct DatabaseFileDeletionPermit(File);

pub(crate) fn open_file(path: &Path, create: bool) -> std::io::Result<File> {
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
        Ok(Self(Mutex::new(Some(file))))
    }
}

impl Database {
    pub fn close_for_reset(&self) -> Result<(), DatabaseError> {
        let mut connection = self.connection.lock().map_err(|_| DatabaseError::Lock)?;
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        let access = self
            .write_access
            .as_ref()
            .ok_or(DatabaseError::WriteFenceRequired)?;
        if !access.fenced()? {
            return Err(DatabaseError::WriteFenceRequired);
        }
        let replacement = rusqlite::Connection::open_in_memory()?;
        let original = std::mem::replace(&mut *connection, replacement);
        match original.close() {
            Ok(()) => {
                self.closed.store(true, Ordering::Release);
                if let Some(file) = &self._file_use {
                    file.close()?;
                }
                Ok(())
            }
            Err((original, error)) => {
                *connection = original;
                Err(DatabaseError::Sql(error))
            }
        }
    }

    pub fn try_reserve_file_deletion(
        path: &Path,
    ) -> Result<Option<DatabaseFileDeletionPermit>, DatabaseError> {
        let file = open_file(path, false)?;
        Ok(FileExt::try_lock_exclusive(&file)?.then_some(DatabaseFileDeletionPermit(file)))
    }
}

impl DatabaseFileUse {
    fn close(&self) -> Result<(), DatabaseError> {
        let file = self.0.lock().map_err(|_| DatabaseError::Lock)?.take();
        if let Some(file) = file {
            FileExt::unlock(&file)?;
        }
        Ok(())
    }
}

impl Drop for DatabaseFileUse {
    fn drop(&mut self) {
        if let Some(file) = self
            .0
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            let _ = FileExt::unlock(&file);
        }
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
