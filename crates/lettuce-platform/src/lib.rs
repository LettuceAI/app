//! Platform lifecycle, managed paths, confined filesystem capabilities, and updates.
//!
//! The filesystem slice is intentionally Tauri-free. The only authority
//! factory is [`FilesystemAuthority`], which belongs at the application
//! composition boundary. Operational consumers receive a [`ManagedFiles`]
//! facade plus purpose-scoped capabilities and cannot mint additional roots or
//! access native paths.

#![deny(unsafe_op_in_unsafe_fn)]

mod atomic;
mod authority;
mod directories;
mod error;
mod keys;
mod managed;
mod model;
mod recovery;
mod trash;

pub use atomic::StagedWrite;
pub use authority::FilesystemAuthority;
pub use directories::{DirectorySnapshot, ManagedRoot, RootKind};
pub use error::PlatformError;
pub use keys::ObjectKey;
pub use managed::ManagedFiles;
pub use model::{
    Atomicity, CommitReceipt, DirectoryEntry, ObjectKind, ObjectMetadata, ParentSyncStatus,
    ReadCapability, ReadHandle, RecoveryReport, StageCleanupStatus, TrashDurability, TrashReceipt,
    TrashRestoreReceipt, WriteCapability, WriteMode,
};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
