use std::{
    fs::File,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::MutexGuard,
};

use fs4::fs_std::FileExt;
use rusqlite::Connection;

use crate::{Database, DatabaseError, database_file::open_file};

pub(crate) fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

pub(crate) struct FileWriteAccess {
    path: PathBuf,
}

pub(crate) struct FileWriteUse(File);

pub(crate) struct DatabaseConnection<'a> {
    connection: MutexGuard<'a, Connection>,
    _write_use: Option<FileWriteUse>,
}

#[derive(Debug)]
pub struct DatabaseWriteFence {
    path: PathBuf,
    gate: File,
    database_lease: File,
}

impl FileWriteAccess {
    pub(crate) fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    pub(crate) fn acquire(&self) -> Result<FileWriteUse, DatabaseError> {
        let gate = open_file(&sidecar(&self.path, ".writes.lock"), true)?;
        FileExt::lock_shared(&gate)?;
        Ok(FileWriteUse(gate))
    }

    pub(crate) fn acquire_exclusive(&self) -> Result<FileWriteUse, DatabaseError> {
        let gate = open_file(&sidecar(&self.path, ".writes.lock"), true)?;
        FileExt::lock_exclusive(&gate)?;
        Ok(FileWriteUse(gate))
    }

    pub(crate) fn fenced(&self) -> Result<bool, DatabaseError> {
        match open_file(&sidecar(&self.path, ".write-fenced"), false) {
            Ok(file) if file.metadata()?.len() == 0 => Ok(true),
            Ok(_) => Err(std::io::Error::from(std::io::ErrorKind::InvalidData).into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

impl<'a> DatabaseConnection<'a> {
    pub(crate) fn new(
        connection: MutexGuard<'a, Connection>,
        access: Option<&FileWriteAccess>,
    ) -> Result<Self, DatabaseError> {
        let write_use = access.map(FileWriteAccess::acquire).transpose()?;
        if let Some(access) = access {
            connection.pragma_update(None, "query_only", access.fenced()?)?;
        }
        Ok(Self {
            connection,
            _write_use: write_use,
        })
    }
}

impl Deref for DatabaseConnection<'_> {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl DerefMut for DatabaseConnection<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

impl Drop for FileWriteUse {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

impl Database {
    pub fn lock_file_writes(path: &Path) -> Result<DatabaseWriteFence, DatabaseError> {
        let database_lease = crate::database_file::use_lock(path)?;
        FileExt::lock_shared(&database_lease)?;
        let gate = open_file(&sidecar(path, ".writes.lock"), true)?;
        FileExt::lock_exclusive(&gate)?;
        Ok(DatabaseWriteFence {
            path: path.to_path_buf(),
            gate,
            database_lease,
        })
    }
}

impl DatabaseWriteFence {
    pub fn set_fenced(&self, fenced: bool) -> Result<(), DatabaseError> {
        let marker = sidecar(&self.path, ".write-fenced");
        if fenced {
            let file = open_file(&marker, true)?;
            if file.metadata()?.len() != 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::InvalidData).into());
            }
            file.sync_all()?;
        } else {
            match std::fs::remove_file(&marker) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
        #[cfg(unix)]
        File::open(
            self.path
                .parent()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?,
        )?
        .sync_all()?;
        Ok(())
    }
}

impl Drop for DatabaseWriteFence {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.gate);
        let _ = FileExt::unlock(&self.database_lease);
    }
}
