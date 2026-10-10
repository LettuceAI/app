use cap_std::fs::OpenOptions;
use fs4::fs_std::FileExt;

use crate::{ManagedFiles, ObjectKey, PlatformError, WriteCapability};

#[derive(Debug)]
pub struct ManagedFileLock {
    _file: std::fs::File,
}

impl Drop for ManagedFileLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self._file);
    }
}

impl ManagedFiles {
    pub fn lock_file(
        &self,
        capability: &WriteCapability,
        key: &ObjectKey,
        wait: bool,
    ) -> Result<Option<ManagedFileLock>, PlatformError> {
        let file = {
            let _mutation = self.mutation_guard()?;
            let root = self.check_write(capability)?;
            let (parent, name) = crate::managed::resolve_parent(root, &key.segments, true)?;
            let mut options = OpenOptions::new();
            options.read(true).write(true).create(true);
            options._cap_fs_ext_follow(cap_primitives::fs::FollowSymlinks::No);
            let file = parent
                .open_with(&name, &options)
                .map_err(crate::authority::map_symlink_error)?
                .into_std();
            if !file.metadata().map_err(PlatformError::from)?.is_file() {
                return Err(PlatformError::Denied);
            }
            file
        };
        let acquired = if wait {
            FileExt::lock_exclusive(&file).map_err(PlatformError::from)?;
            true
        } else {
            FileExt::try_lock_exclusive(&file).map_err(PlatformError::from)?
        };
        Ok(acquired.then_some(ManagedFileLock { _file: file }))
    }
}
