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
mod database_files;
mod directories;
mod error;
mod espeak;
#[cfg(any(target_os = "android", test))]
mod espeak_library;
mod file_lock;
mod install;
mod keys;
mod managed;
mod model;
mod memory;
mod memory_pressure;
mod recovery;
mod trash;

pub use atomic::StagedWrite;
pub use authority::FilesystemAuthority;
pub use database_files::DatabaseFiles;
pub use directories::{DirectorySnapshot, ManagedRoot, RootKind};
pub use error::PlatformError;
pub use espeak::{EspeakNgError, EspeakNgProcess, EspeakPhonemizer};
#[cfg(target_os = "android")]
pub use espeak_library::EspeakNgLibrary;
pub use file_lock::ManagedFileLock;
pub use install::{
    ConfinedDirectoryEntry, ConfinedInstallStore, InstallPreparation, InstalledFile,
    ResumableInstall,
};
pub use keys::ObjectKey;
pub use managed::{MAX_MANAGED_READ_BYTES, ManagedFiles};
pub use model::{
    Atomicity, CommitReceipt, DirectoryEntry, ObjectKind, ObjectMetadata, ParentSyncStatus,
    ReadCapability, ReadHandle, RecoveryReport, StageCleanupStatus, TrashDurability, TrashReceipt,
    TrashRestoreReceipt, WriteCapability, WriteMode,
};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
mod file_lock_tests;

#[cfg(test)]
mod database_files_tests;

pub use memory::{MemorySnapshot, system_memory};

pub use memory_pressure::MemoryPressureMonitor;
