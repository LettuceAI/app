use std::{
    fmt,
    path::{Component, Path, PathBuf},
};

use crate::error::PlatformError;

pub(crate) const PLATFORM_DIRECTORY: &str = "platform-v2";

/// Only non-overlapping leaf roots are grantable. The platform container is an
/// internal implementation detail and is never represented as a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManagedRoot {
    Diagnostics,
    ImportStaging,
    JobStaging,
    MediaBlobs,
    Quarantine,
    Trash,
    PrivatePersistent,
}

pub use ManagedRoot as RootKind;

impl ManagedRoot {
    pub(crate) const ALL: [Self; 7] = [
        Self::Diagnostics,
        Self::ImportStaging,
        Self::JobStaging,
        Self::MediaBlobs,
        Self::Quarantine,
        Self::Trash,
        Self::PrivatePersistent,
    ];

    pub(crate) const fn directory(self) -> &'static str {
        match self {
            Self::Diagnostics => "diagnostics",
            Self::ImportStaging => "import-staging",
            Self::JobStaging => "job-staging",
            Self::MediaBlobs => "media-blobs",
            Self::Quarantine => "quarantine",
            Self::Trash => "trash",
            Self::PrivatePersistent => "private-persistent-v2",
        }
    }
}

/// Immutable, app-resolved locations. This is an adapter construction
/// boundary: the composition root supplies trusted native locations once, and
/// no accessor returns those locations to a domain consumer. Existing ambient
/// ancestors are trusted by that adapter; the authority then opens the final
/// roots without following final symlinks and all operations below them are
/// descriptor-relative.
pub struct DirectorySnapshot {
    pub(crate) app_data: PathBuf,
    pub(crate) private_persistent: PathBuf,
}

impl DirectorySnapshot {
    pub fn new(app_data: impl Into<PathBuf>) -> Result<Self, PlatformError> {
        let app_data = app_data.into();
        Self::with_private_persistent(app_data.clone(), app_data.join("private-persistent-v2"))
    }

    pub fn from_app_data(app_data: impl Into<PathBuf>) -> Result<Self, PlatformError> {
        Self::new(app_data)
    }

    pub fn with_private_persistent(
        app_data: impl Into<PathBuf>,
        private_persistent: impl Into<PathBuf>,
    ) -> Result<Self, PlatformError> {
        let app_data = app_data.into();
        if !valid_root_path(&app_data) {
            return Err(PlatformError::Denied);
        }
        let private_persistent = private_persistent.into();
        let platform = app_data.join(PLATFORM_DIRECTORY);
        if !valid_root_path(&private_persistent)
            || private_persistent == app_data
            || private_persistent.starts_with(&platform)
            || platform.starts_with(&private_persistent)
        {
            return Err(PlatformError::Denied);
        }
        Ok(Self {
            app_data,
            private_persistent,
        })
    }
}

impl Clone for DirectorySnapshot {
    fn clone(&self) -> Self {
        Self {
            app_data: self.app_data.clone(),
            private_persistent: self.private_persistent.clone(),
        }
    }
}

impl fmt::Debug for DirectorySnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DirectorySnapshot")
            .field("grantable_leaf_roots", &ManagedRoot::ALL.len())
            .finish()
    }
}

fn valid_root_path(path: &Path) -> bool {
    path.is_absolute()
        && path.components().count() > 1
        && !path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
}
