use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::Path,
    sync::{Arc, Mutex},
};

use cap_primitives::fs::{FollowSymlinks, open_ambient, open_dir_nofollow};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};

use crate::{
    directories::{DirectorySnapshot, ManagedRoot, PLATFORM_DIRECTORY},
    error::PlatformError,
    managed::ManagedFiles,
    model::{ReadCapability, WriteCapability},
};

/// The only public authority factory. The application composition root should
/// construct this once from its attested directory snapshot and pass the
/// resulting managed facade and purpose capabilities to consumers.
#[derive(Clone)]
pub struct FilesystemAuthority {
    pub(crate) inner: Arc<AuthorityInner>,
}

pub(crate) struct AuthorityInner {
    pub(crate) roots: BTreeMap<ManagedRoot, RootHandle>,
    pub(crate) mutation_lock: Mutex<()>,
}

pub(crate) struct RootHandle {
    pub(crate) dir: Arc<Dir>,
}

impl fmt::Debug for FilesystemAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FilesystemAuthority")
            .field("grantable_leaf_count", &self.inner.roots.len())
            .finish()
    }
}

impl FilesystemAuthority {
    /// Construct the managed authority at the app/platform adapter boundary.
    /// Native paths are accepted here only; they are never returned and are
    /// not accepted by operational methods.
    pub fn new(snapshot: &DirectorySnapshot) -> Result<Self, PlatformError> {
        let app_data = open_or_create_ambient_root(&snapshot.app_data)?;
        let platform = open_or_create_child(&app_data, PLATFORM_DIRECTORY)?;
        let private =
            if snapshot.private_persistent == snapshot.app_data.join("private-persistent-v2") {
                open_or_create_child(&app_data, "private-persistent-v2")?
            } else {
                open_or_create_ambient_root(&snapshot.private_persistent)?
            };

        let mut roots = BTreeMap::new();
        for root in ManagedRoot::ALL {
            let dir = if root == ManagedRoot::PrivatePersistent {
                private.try_clone().map_err(PlatformError::from)?
            } else {
                open_or_create_child(&platform, root.directory())?
            };
            roots.insert(root, RootHandle { dir: Arc::new(dir) });
        }
        Ok(Self {
            inner: Arc::new(AuthorityInner {
                roots,
                mutation_lock: Mutex::new(()),
            }),
        })
    }

    pub fn read_capability(&self, root: ManagedRoot) -> Result<ReadCapability, PlatformError> {
        self.ensure_root(root)?;
        Ok(ReadCapability {
            inner: Arc::clone(&self.inner),
            root,
        })
    }

    pub fn write_capability(&self, root: ManagedRoot) -> Result<WriteCapability, PlatformError> {
        self.ensure_root(root)?;
        Ok(WriteCapability {
            inner: Arc::clone(&self.inner),
            root,
        })
    }

    pub fn managed_files(&self) -> ManagedFiles {
        ManagedFiles::from_authority(Arc::clone(&self.inner))
    }

    fn ensure_root(&self, root: ManagedRoot) -> Result<(), PlatformError> {
        self.inner
            .roots
            .contains_key(&root)
            .then_some(())
            .ok_or(PlatformError::InvalidRoot)
    }
}

pub(crate) fn open_or_create_ambient_root(path: &Path) -> Result<Dir, PlatformError> {
    if !path.is_absolute() {
        return Err(PlatformError::Denied);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(PlatformError::from)?;
    }
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(PlatformError::from(error)),
    }

    // The adapter is responsible for ambient ancestor attestation. This final
    // open is explicitly no-follow; all subsequent operations use this handle.
    let mut options = OpenOptions::new();
    options.read(true);
    options._cap_fs_ext_follow(FollowSymlinks::No);
    let file = open_ambient(path, &options, ambient_authority()).map_err(map_symlink_error)?;
    if !file.metadata().map_err(PlatformError::from)?.is_dir() {
        return Err(PlatformError::Denied);
    }
    Ok(Dir::from_std_file(file))
}

pub(crate) fn open_or_create_child(parent: &Dir, name: &str) -> Result<Dir, PlatformError> {
    let name = Path::new(name);
    match open_dir_nofollow_from(parent, name) {
        Ok(dir) => Ok(dir),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match parent.create_dir(name) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(PlatformError::from(error)),
            }
            open_dir_nofollow_from(parent, name).map_err(map_symlink_error)
        }
        Err(error) => Err(map_symlink_error(error)),
    }
}

pub(crate) fn open_dir_nofollow_from(parent: &Dir, name: &Path) -> io::Result<Dir> {
    let native = parent.try_clone()?.into_std_file();
    let child = open_dir_nofollow(&native, name)?;
    Ok(Dir::from_std_file(child))
}

pub(crate) fn map_symlink_error(error: io::Error) -> PlatformError {
    if matches!(
        error.kind(),
        io::ErrorKind::InvalidInput | io::ErrorKind::PermissionDenied
    ) {
        PlatformError::SymlinkEscape
    } else {
        PlatformError::from(error)
    }
}
