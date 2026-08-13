use std::{
    fmt,
    fs::File,
    io::{self, Read},
    sync::{Arc, Weak},
};

use crate::{authority::AuthorityInner, directories::ManagedRoot, keys::ObjectKey};

#[derive(Clone)]
pub struct ReadCapability {
    pub(crate) inner: Arc<AuthorityInner>,
    pub(crate) root: ManagedRoot,
}

#[derive(Clone)]
pub struct WriteCapability {
    pub(crate) inner: Arc<AuthorityInner>,
    pub(crate) root: ManagedRoot,
}

impl fmt::Debug for ReadCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ReadCapability(<managed leaf>)")
    }
}

impl fmt::Debug for WriteCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WriteCapability(<managed leaf>)")
    }
}

/// A descriptor-backed read handle that cannot be converted to a native path
/// through this crate's API.
pub struct ReadHandle {
    pub(crate) file: File,
}

impl Read for ReadHandle {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file.read(buffer)
    }
}

impl fmt::Debug for ReadHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ReadHandle(<managed file>)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    File,
    Directory,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMetadata {
    pub kind: ObjectKind,
    pub len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub kind: ObjectKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    CreateNew,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atomicity {
    Atomic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentSyncStatus {
    Synced,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageCleanupStatus {
    Cleaned,
    RecoveryNeeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitReceipt {
    pub atomicity: Atomicity,
    pub parent_sync: ParentSyncStatus,
    pub stage_cleanup: StageCleanupStatus,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrashDurability {
    pub source_parent_sync: ParentSyncStatus,
    pub trash_parent_sync: ParentSyncStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrashRestoreReceipt {
    pub destination_parent_sync: ParentSyncStatus,
    pub trash_parent_sync: ParentSyncStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecoveryReport {
    pub inspected: usize,
    pub retained: usize,
    pub truncated: bool,
}

/// Opaque, authority-bound receipt for a managed move-to-trash operation.
/// Keeping the authority weak reference prevents a receipt minted by another
/// authority from being replayed against this one.
pub struct TrashReceipt {
    pub(crate) id: uuid::Uuid,
    pub(crate) authority: Weak<AuthorityInner>,
    pub(crate) origin: ManagedRoot,
    pub(crate) key: ObjectKey,
    pub(crate) trash_name: String,
    pub(crate) durability: TrashDurability,
}

impl TrashReceipt {
    pub fn id(&self) -> uuid::Uuid {
        self.id
    }

    pub fn durability(&self) -> TrashDurability {
        self.durability
    }
}

impl fmt::Debug for TrashReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TrashReceipt(<opaque>)")
    }
}
