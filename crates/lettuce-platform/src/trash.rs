use std::{io, path::Path, sync::Arc};

use uuid::Uuid;

use crate::{
    directories::ManagedRoot,
    error::PlatformError,
    keys::{TRASH_PREFIX, is_owned_trash_name},
    managed::{ManagedFiles, resolve_parent, sync_directory},
    model::{TrashDurability, TrashReceipt, TrashRestoreReceipt, WriteCapability},
};

impl ManagedFiles {
    pub fn remove_to_trash(
        &self,
        capability: &WriteCapability,
        key: crate::ObjectKey,
    ) -> Result<TrashReceipt, PlatformError> {
        if capability.root == ManagedRoot::PrivatePersistent {
            return Err(PlatformError::Unsupported);
        }
        let _guard = self.mutation_guard()?;
        let source_root = self.check_write(capability)?;
        let source_parent = resolve_parent(source_root, &key.segments, false)?;
        source_parent
            .0
            .symlink_metadata(&source_parent.1)
            .map_err(crate::authority::map_symlink_error)?;
        let trash = self
            .inner
            .roots
            .get(&ManagedRoot::Trash)
            .ok_or(PlatformError::InvalidRoot)?;
        let trash_parent = trash.dir.try_clone().map_err(PlatformError::from)?;
        let trash_name = format!("{TRASH_PREFIX}{}", Uuid::new_v4().simple());
        source_parent
            .0
            .rename(&source_parent.1, &trash_parent, Path::new(&trash_name))
            .map_err(|error| {
                if error.kind() == io::ErrorKind::CrossesDevices {
                    PlatformError::Unsupported
                } else {
                    PlatformError::from(error)
                }
            })?;
        let source_parent_sync = sync_directory(&source_parent.0);
        let trash_parent_sync = sync_directory(&trash_parent);
        Ok(TrashReceipt {
            id: Uuid::new_v4(),
            authority: Arc::downgrade(&self.inner),
            origin: capability.root,
            key,
            trash_name,
            durability: TrashDurability {
                source_parent_sync,
                trash_parent_sync,
            },
        })
    }

    pub fn restore_from_trash(
        &self,
        capability: &WriteCapability,
        receipt: &TrashReceipt,
    ) -> Result<TrashRestoreReceipt, PlatformError> {
        let receipt_authority = receipt
            .authority
            .upgrade()
            .ok_or(PlatformError::WrongCapability)?;
        if !Arc::ptr_eq(&receipt_authority, &self.inner) {
            return Err(PlatformError::WrongCapability);
        }
        if !is_owned_trash_name(&receipt.trash_name) || capability.root != receipt.origin {
            return Err(PlatformError::WrongCapability);
        }
        let _guard = self.mutation_guard()?;
        let destination_root = self.check_write(capability)?;
        let trash = self
            .inner
            .roots
            .get(&ManagedRoot::Trash)
            .ok_or(PlatformError::InvalidRoot)?;
        let trash_parent = trash.dir.try_clone().map_err(PlatformError::from)?;
        let destination = resolve_parent(destination_root, &receipt.key.segments, true)?;
        if destination.0.symlink_metadata(&destination.1).is_ok() {
            return Err(PlatformError::Conflict);
        }
        trash_parent
            .rename(
                Path::new(&receipt.trash_name),
                &destination.0,
                &destination.1,
            )
            .map_err(PlatformError::from)?;
        let trash_parent_sync = sync_directory(&trash_parent);
        let destination_parent_sync = sync_directory(&destination.0);
        Ok(TrashRestoreReceipt {
            destination_parent_sync,
            trash_parent_sync,
        })
    }
}
