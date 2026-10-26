use std::{
    io::{Seek, SeekFrom, Write},
    path::Path,
};

use lettuce_app::api::{
    ExportProtection, FileAccess, FileAccessError, FileDescription, FileReader, PickFilter,
};
use tauri::{AppHandle, Runtime};

fn access_error(error: &std::io::Error) -> FileAccessError {
    match error.kind() {
        std::io::ErrorKind::NotFound => FileAccessError::NotFound,
        std::io::ErrorKind::PermissionDenied => FileAccessError::PermissionDenied,
        _ => FileAccessError::Io,
    }
}

struct ExportTarget(std::fs::File);

impl Write for ExportTarget {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl Drop for ExportTarget {
    fn drop(&mut self) {
        let _ = fs4::fs_std::FileExt::unlock(&self.0);
    }
}

fn path(uri: &str) -> Result<&Path, FileAccessError> {
    if uri.contains("://") {
        return Err(FileAccessError::Unsupported);
    }
    Ok(Path::new(uri))
}

fn describe_path(uri: &str) -> Result<FileDescription, FileAccessError> {
    let path = path(uri)?;
    let metadata = std::fs::metadata(path).map_err(|error| access_error(&error))?;
    if !metadata.is_file() {
        return Err(FileAccessError::Unsupported);
    }
    Ok(FileDescription {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size: metadata.len(),
    })
}

fn open_path(uri: &str) -> Result<Box<dyn FileReader>, FileAccessError> {
    std::fs::File::open(path(uri)?)
        .map(|file| Box::new(file) as Box<dyn FileReader>)
        .map_err(|error| access_error(&error))
}

fn create_path(uri: &str) -> Result<Box<dyn Write + Send>, FileAccessError> {
    std::fs::File::create(path(uri)?)
        .map(|file| Box::new(file) as Box<dyn Write + Send>)
        .map_err(|error| access_error(&error))
}

fn prepare_export(
    uri: &str,
    target: std::fs::File,
    source: Option<&std::fs::File>,
    protected: ExportProtection<'_>,
) -> Result<Box<dyn Write + Send>, FileAccessError> {
    let target_handle =
        same_file::Handle::from_file(target.try_clone().map_err(|error| access_error(&error))?)
            .map_err(|error| access_error(&error))?;
    if let Some(source) = source {
        let source_handle =
            same_file::Handle::from_file(source.try_clone().map_err(|error| access_error(&error))?)
                .map_err(|error| access_error(&error))?;
        if source_handle == target_handle {
            return Err(FileAccessError::SourceIsTarget);
        }
    }
    if protected(uri, Some(&target))? {
        return Err(FileAccessError::SourceIsTarget);
    }
    if !fs4::fs_std::FileExt::try_lock_exclusive(&target).map_err(|error| access_error(&error))? {
        return Err(FileAccessError::Busy);
    }
    let mut target = ExportTarget(target);
    target.0.set_len(0).map_err(|error| access_error(&error))?;
    target
        .0
        .seek(SeekFrom::Start(0))
        .map_err(|error| access_error(&error))?;
    Ok(Box::new(target))
}

fn create_export_path(
    uri: &str,
    source: Option<&std::fs::File>,
    protected: ExportProtection<'_>,
) -> Result<Box<dyn Write + Send>, FileAccessError> {
    if protected(uri, None)? {
        return Err(FileAccessError::SourceIsTarget);
    }
    let target = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path(uri)?)
        .map_err(|error| access_error(&error))?;
    prepare_export(uri, target, source, protected)
}

#[cfg(not(target_os = "android"))]
fn backup_target_path(target: &str) -> Result<std::path::PathBuf, FileAccessError> {
    let target = path(target)?;
    let name = target.file_name().ok_or(FileAccessError::Unsupported)?;
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok(parent
        .canonicalize()
        .map_err(|failure| access_error(&failure))?
        .join(name))
}

#[cfg(not(target_os = "android"))]
fn backup_temporary_location_path(
    target: Option<&str>,
    name: &str,
) -> Result<String, FileAccessError> {
    let target = backup_target_path(target.ok_or(FileAccessError::Unsupported)?)?;
    if Path::new(name).file_name() != Some(std::ffi::OsStr::new(name)) {
        return Err(FileAccessError::Unsupported);
    }
    target
        .with_file_name(name)
        .into_os_string()
        .into_string()
        .map_err(|_| FileAccessError::Unsupported)
}

#[cfg(not(target_os = "android"))]
fn create_backup_temporary_path(
    intent: &str,
    protected: ExportProtection<'_>,
) -> Result<(String, Box<dyn Write + Send>), FileAccessError> {
    if protected(intent, None)? {
        return Err(FileAccessError::SourceIsTarget);
    }
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path(intent)?)
        .map_err(|failure| access_error(&failure))?;
    Ok((intent.to_owned(), Box::new(file)))
}

#[cfg(not(target_os = "android"))]
fn publish_backup_path(
    temporary: &str,
    target: Option<&str>,
    protected: ExportProtection<'_>,
) -> Result<String, FileAccessError> {
    let target_path = backup_target_path(target.ok_or(FileAccessError::Unsupported)?)?;
    let target = target_path.to_str().ok_or(FileAccessError::Unsupported)?;
    if protected(target, None)? {
        return Err(FileAccessError::SourceIsTarget);
    }
    let existing = match std::fs::File::open(path(target)?) {
        Ok(file) => Some(file),
        Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => None,
        Err(failure) => return Err(access_error(&failure)),
    };
    if let Some(file) = &existing
        && protected(target, Some(file))?
    {
        return Err(FileAccessError::SourceIsTarget);
    }
    publish_backup_durable(path(temporary)?, path(target)?)?;
    Ok(target.to_owned())
}

#[cfg(not(target_os = "android"))]
fn publish_backup_durable(temporary: &Path, target: &Path) -> Result<(), FileAccessError> {
    std::fs::rename(temporary, target).map_err(|failure| access_error(&failure))?;
    #[cfg(not(target_os = "windows"))]
    {
        let parent = target
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|failure| access_error(&failure))?;
    }
    Ok(())
}

/// Files the desktop dialogs return: filesystem paths, opened with `std::fs`.
#[cfg(desktop)]
pub(crate) struct DesktopFileAccess<R: Runtime>(pub(crate) AppHandle<R>);

#[cfg(desktop)]
impl<R: Runtime> DesktopFileAccess<R> {
    fn dialog(&self, filter: &PickFilter) -> tauri_plugin_dialog::FileDialogBuilder<R> {
        use tauri_plugin_dialog::DialogExt;
        let dialog = self.0.dialog().file();
        if filter.extensions.is_empty() {
            return dialog;
        }
        dialog.add_filter(filter.extensions.join(", "), &filter.extensions)
    }
}

#[cfg(desktop)]
fn picked_path(picked: tauri_plugin_dialog::FilePath) -> Result<String, FileAccessError> {
    picked
        .into_path()
        .map_err(|_| FileAccessError::Unsupported)?
        .into_os_string()
        .into_string()
        .map_err(|_| FileAccessError::Unsupported)
}

#[cfg(desktop)]
impl<R: Runtime> FileAccess for DesktopFileAccess<R> {
    fn backup_temporary_location(
        &self,
        target: Option<&str>,
        name: &str,
    ) -> Result<String, FileAccessError> {
        backup_temporary_location_path(target, name)
    }
    fn create_backup_temporary(
        &self,
        intent: &str,
        protected: ExportProtection<'_>,
    ) -> Result<(String, Box<dyn Write + Send>), FileAccessError> {
        create_backup_temporary_path(intent, protected)
    }
    fn sync_backup(&self, uri: &str) -> Result<(), FileAccessError> {
        std::fs::File::open(path(uri)?)
            .and_then(|file| file.sync_all())
            .map_err(|failure| access_error(&failure))
    }
    fn publish_backup(
        &self,
        temporary: &str,
        target: Option<&str>,
        _name: &str,
        protected: ExportProtection<'_>,
    ) -> Result<String, FileAccessError> {
        publish_backup_path(temporary, target, protected)
    }

    fn remove(&self, uri: &str) -> Result<(), FileAccessError> {
        remove_path(uri)
    }

    fn describe(&self, uri: &str) -> Result<FileDescription, FileAccessError> {
        describe_path(uri)
    }

    fn open(&self, uri: &str) -> Result<Box<dyn FileReader>, FileAccessError> {
        open_path(uri)
    }

    fn create(&self, uri: &str) -> Result<Box<dyn Write + Send>, FileAccessError> {
        create_path(uri)
    }

    fn create_export(
        &self,
        uri: &str,
        source: Option<&std::fs::File>,
        protected: ExportProtection<'_>,
    ) -> Result<Box<dyn Write + Send>, FileAccessError> {
        create_export_path(uri, source, protected)
    }

    fn pick_open(
        &self,
        filter: &PickFilter,
        multiple: bool,
    ) -> Result<Vec<String>, FileAccessError> {
        let picked = if multiple {
            self.dialog(filter)
                .blocking_pick_files()
                .unwrap_or_default()
        } else {
            self.dialog(filter)
                .blocking_pick_file()
                .into_iter()
                .collect()
        };
        picked.into_iter().map(picked_path).collect()
    }

    fn pick_save(
        &self,
        suggested_name: &str,
        filter: &PickFilter,
    ) -> Result<Option<String>, FileAccessError> {
        self.dialog(filter)
            .set_file_name(suggested_name)
            .blocking_save_file()
            .map(picked_path)
            .transpose()
    }

    fn pick_directory(&self) -> Result<Option<String>, FileAccessError> {
        self.dialog(&PickFilter::default())
            .blocking_pick_folder()
            .map(picked_path)
            .transpose()
    }
}

/// Files on Android: `content://` and `file://` URIs from the system
/// pickers go through the Storage Access Framework, plain paths into the
/// app's own folders through `std::fs`.
#[cfg(target_os = "android")]
pub(crate) struct AndroidFileAccess<R: Runtime>(pub(crate) AppHandle<R>);

#[cfg(target_os = "android")]
impl<R: Runtime> AndroidFileAccess<R> {
    fn fs(&self) -> &tauri_plugin_android_fs::api::api_sync::AndroidFs<R> {
        use tauri_plugin_android_fs::AndroidFsExt;
        self.0.android_fs()
    }

    #[allow(deprecated)]
    fn backup_directory(&self) -> Result<std::path::PathBuf, FileAccessError> {
        Ok(self
            .fs()
            .public_storage()
            .resolve_path(
                None,
                tauri_plugin_android_fs::PublicGeneralPurposeDir::Download,
            )
            .map_err(platform_error)?
            .join("LettuceAI"))
    }

    fn metadata_error(&self) -> FileAccessError {
        let public = self.fs().public_storage();
        let permission = match public.check_permission() {
            Ok(permission) => permission,
            Err(_) => return FileAccessError::Io,
        };
        if !permission {
            return FileAccessError::PermissionDenied;
        }
        let available = match public.get_primary_volume() {
            Ok(volume) => volume.is_some_and(|volume| volume.is_available_for_public_storage),
            Err(_) => return FileAccessError::Io,
        };
        classify_backup_metadata_error(permission, available)
    }

    fn pending_backup_uri(
        &self,
        intent: &str,
    ) -> Result<tauri_plugin_android_fs::FsUri, FileAccessError> {
        let public = self.fs().public_storage();
        if !public.check_permission().map_err(platform_error)? {
            return Err(FileAccessError::PermissionDenied);
        }
        if !public
            .get_primary_volume()
            .map_err(platform_error)?
            .is_some_and(|volume| volume.is_available_for_public_storage)
        {
            return Err(FileAccessError::Io);
        }
        let directory = self.backup_directory()?;
        let intent = path(intent)?;
        if intent.parent() != Some(directory.as_path())
            || !intent.file_name().is_some_and(|name| {
                name.to_string_lossy().starts_with("lettuce-backup-")
                    && name.to_string_lossy().ends_with(".partial")
            })
        {
            return Err(FileAccessError::Unsupported);
        }
        self.fs()
            .get_info(&tauri_plugin_android_fs::FsUri::from_path(&intent))
            .map_err(|_| self.metadata_error())?;
        public
            .scan_by_path(intent, Some("application/octet-stream"))
            .map_err(platform_error)
    }
}

#[cfg(target_os = "android")]
fn platform_error(error: tauri_plugin_android_fs::Error) -> FileAccessError {
    access_error(&std::io::Error::from(error))
}

#[cfg(target_os = "android")]
fn platform_uri(uri: &str) -> Option<tauri_plugin_android_fs::FsUri> {
    uri.contains("://")
        .then(|| tauri_plugin_android_fs::FsUri::from_uri(uri))
}

#[cfg(target_os = "android")]
impl<R: Runtime> FileAccess for AndroidFileAccess<R> {
    fn backup_temporary_location(
        &self,
        target: Option<&str>,
        name: &str,
    ) -> Result<String, FileAccessError> {
        if target.is_some() || Path::new(name).file_name() != Some(std::ffi::OsStr::new(name)) {
            return Err(FileAccessError::Unsupported);
        }
        Ok(self
            .backup_directory()?
            .join(name)
            .to_string_lossy()
            .into_owned())
    }

    fn create_backup_temporary(
        &self,
        intent: &str,
        _protected: ExportProtection<'_>,
    ) -> Result<(String, Box<dyn Write + Send>), FileAccessError> {
        use tauri_plugin_android_fs::PublicGeneralPurposeDir;
        let public = self.fs().public_storage();
        if !public.check_permission().map_err(platform_error)? {
            return Err(FileAccessError::PermissionDenied);
        }
        if !public
            .get_primary_volume()
            .map_err(platform_error)?
            .is_some_and(|volume| volume.is_available_for_public_storage)
        {
            return Err(FileAccessError::Io);
        }
        let directory = self.backup_directory()?;
        let intent = path(intent)?;
        if intent.parent() != Some(directory.as_path()) {
            return Err(FileAccessError::Unsupported);
        }
        if self
            .fs()
            .get_info(&tauri_plugin_android_fs::FsUri::from_path(&intent))
            .is_ok()
        {
            return Err(FileAccessError::Busy);
        }
        match self.metadata_error() {
            FileAccessError::NotFound => {}
            failure => return Err(failure),
        }
        let name = intent.file_name().ok_or(FileAccessError::Unsupported)?;
        let uri = public
            .create_new_file_with_pending(
                None,
                PublicGeneralPurposeDir::Download,
                Path::new("LettuceAI").join(name),
                Some("application/octet-stream"),
            )
            .map_err(platform_error)?;
        let writer = match self.fs().open_file_writable(&uri) {
            Ok(writer) => writer,
            Err(failure) => {
                self.fs().remove_file(&uri).map_err(platform_error)?;
                return Err(platform_error(failure));
            }
        };
        Ok((uri.uri.clone(), Box::new(writer)))
    }

    fn sync_backup(&self, uri: &str) -> Result<(), FileAccessError> {
        let uri = platform_uri(uri).ok_or(FileAccessError::Unsupported)?;
        self.fs()
            .open_file_readable(&uri)
            .map_err(platform_error)?
            .sync_all()
            .map_err(|failure| access_error(&failure))
    }

    fn backup_publication_candidates(
        &self,
        temporary: &str,
        name: &str,
    ) -> Result<Vec<String>, FileAccessError> {
        use tauri_plugin_android_fs::FsUri;
        let uri = platform_uri(temporary).ok_or(FileAccessError::Unsupported)?;
        if self.fs().open_file_readable(&uri).is_ok() {
            return Ok(vec![temporary.to_owned()]);
        }
        match self.metadata_error() {
            FileAccessError::NotFound => {}
            failure => return Err(failure),
        }
        let public = self.fs().public_storage();
        let directory = self.backup_directory()?;
        let entries = match self.fs().read_dir(&FsUri::from_path(&directory)) {
            Ok(entries) => entries,
            Err(_) => {
                return match self.metadata_error() {
                    FileAccessError::NotFound => Ok(Vec::new()),
                    failure => Err(failure),
                };
            }
        };
        let stem = Path::new(name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or(FileAccessError::Unsupported)?;
        let mut candidates = Vec::new();
        for entry in entries {
            if entry.is_file()
                && entry.name().starts_with(stem)
                && entry.name().ends_with(".lettuce")
            {
                candidates.push(
                    public
                        .scan_by_path(
                            directory.join(entry.name()),
                            Some("application/octet-stream"),
                        )
                        .map_err(platform_error)?
                        .uri,
                );
            }
        }
        Ok(candidates)
    }

    fn publish_backup(
        &self,
        temporary: &str,
        target: Option<&str>,
        name: &str,
        _protected: ExportProtection<'_>,
    ) -> Result<String, FileAccessError> {
        if target.is_some() {
            return Err(FileAccessError::Unsupported);
        }
        let uri = platform_uri(temporary).ok_or(FileAccessError::Unsupported)?;
        let published = if self
            .fs()
            .get_name(&uri)
            .map_err(platform_error)?
            .ends_with(".partial")
        {
            self.fs().rename(&uri, name).map_err(platform_error)?
        } else {
            uri
        };
        self.fs()
            .public_storage()
            .set_pending(&published, false)
            .map_err(platform_error)?;
        Ok(published.uri)
    }

    fn remove(&self, uri: &str) -> Result<(), FileAccessError> {
        let uri = match platform_uri(uri) {
            Some(uri) => uri,
            None if Path::new(uri).file_name().is_some_and(|name| {
                name.to_string_lossy().starts_with("lettuce-backup-")
                    && name.to_string_lossy().ends_with(".partial")
            }) =>
            {
                self.pending_backup_uri(uri)?
            }
            None => return remove_path(uri),
        };
        self.fs().remove_file(&uri).map_err(|failure| {
            if self.fs().get_info(&uri).is_err() {
                self.metadata_error()
            } else {
                platform_error(failure)
            }
        })
    }

    fn describe(&self, uri: &str) -> Result<FileDescription, FileAccessError> {
        let uri = match platform_uri(uri) {
            Some(uri) => uri,
            None if Path::new(uri).file_name().is_some_and(|name| {
                name.to_string_lossy().starts_with("lettuce-backup-")
                    && name.to_string_lossy().ends_with(".partial")
            }) =>
            {
                self.pending_backup_uri(uri)?
            }
            None => return describe_path(uri),
        };
        let entry = self
            .fs()
            .get_info(&uri)
            .map_err(|_| self.metadata_error())?;
        Ok(FileDescription {
            name: entry.name().to_owned(),
            size: entry.file_len_or_err().map_err(platform_error)?,
        })
    }

    fn open(&self, uri: &str) -> Result<Box<dyn FileReader>, FileAccessError> {
        let Some(uri) = platform_uri(uri) else {
            return open_path(uri);
        };
        self.fs()
            .open_file_readable(&uri)
            .map(|file| Box::new(file) as Box<dyn FileReader>)
            .map_err(platform_error)
    }

    fn create(&self, uri: &str) -> Result<Box<dyn Write + Send>, FileAccessError> {
        let Some(uri) = platform_uri(uri) else {
            return create_path(uri);
        };
        self.fs()
            .open_file_writable(&uri)
            .map(|file| Box::new(file) as Box<dyn Write + Send>)
            .map_err(platform_error)
    }

    fn create_export(
        &self,
        uri: &str,
        source: Option<&std::fs::File>,
        protected: ExportProtection<'_>,
    ) -> Result<Box<dyn Write + Send>, FileAccessError> {
        let Some(uri) = platform_uri(uri) else {
            return create_export_path(uri, source, protected);
        };
        if protected(&uri.uri, None)? {
            return Err(FileAccessError::SourceIsTarget);
        }
        let target = self
            .fs()
            .open_file(&uri, tauri_plugin_android_fs::FileAccessMode::ReadWrite)
            .map_err(platform_error)?;
        prepare_export(&uri.uri, target, source, protected)
    }

    fn pick_open(
        &self,
        filter: &PickFilter,
        multiple: bool,
    ) -> Result<Vec<String>, FileAccessError> {
        let picker = self.fs().picker();
        let picked = if multiple {
            picker
                .pick_files(None, &filter.mime_types, false)
                .map_err(platform_error)?
        } else {
            picker
                .pick_file(None, &filter.mime_types, false)
                .map_err(platform_error)?
                .into_iter()
                .collect()
        };
        Ok(picked.into_iter().map(|uri| uri.uri).collect())
    }

    fn pick_save(
        &self,
        suggested_name: &str,
        filter: &PickFilter,
    ) -> Result<Option<String>, FileAccessError> {
        let mime_type = match filter.mime_types.as_slice() {
            [mime_type] if !mime_type.contains('*') => Some(*mime_type),
            _ => None,
        };
        self.fs()
            .picker()
            .save_file(None, suggested_name, mime_type, false)
            .map(|uri| uri.map(|uri| uri.uri))
            .map_err(platform_error)
    }
}

/// Files on a platform with no picker in this shell: plain paths only.
#[cfg(not(any(desktop, target_os = "android")))]
pub(crate) struct PathFileAccess;

#[cfg(not(any(desktop, target_os = "android")))]
impl FileAccess for PathFileAccess {
    fn backup_temporary_location(
        &self,
        target: Option<&str>,
        name: &str,
    ) -> Result<String, FileAccessError> {
        backup_temporary_location_path(target, name)
    }
    fn create_backup_temporary(
        &self,
        intent: &str,
        protected: ExportProtection<'_>,
    ) -> Result<(String, Box<dyn Write + Send>), FileAccessError> {
        create_backup_temporary_path(intent, protected)
    }
    fn sync_backup(&self, uri: &str) -> Result<(), FileAccessError> {
        std::fs::File::open(path(uri)?)
            .and_then(|file| file.sync_all())
            .map_err(|failure| access_error(&failure))
    }
    fn publish_backup(
        &self,
        temporary: &str,
        target: Option<&str>,
        _name: &str,
        protected: ExportProtection<'_>,
    ) -> Result<String, FileAccessError> {
        publish_backup_path(temporary, target, protected)
    }

    fn remove(&self, uri: &str) -> Result<(), FileAccessError> {
        remove_path(uri)
    }

    fn describe(&self, uri: &str) -> Result<FileDescription, FileAccessError> {
        describe_path(uri)
    }

    fn open(&self, uri: &str) -> Result<Box<dyn FileReader>, FileAccessError> {
        open_path(uri)
    }

    fn create(&self, uri: &str) -> Result<Box<dyn Write + Send>, FileAccessError> {
        create_path(uri)
    }

    fn create_export(
        &self,
        uri: &str,
        source: Option<&std::fs::File>,
        protected: ExportProtection<'_>,
    ) -> Result<Box<dyn Write + Send>, FileAccessError> {
        create_export_path(uri, source, protected)
    }
}

/// The file access this platform's shell provides.
pub(crate) fn file_access<R: Runtime>(handle: &AppHandle<R>) -> std::sync::Arc<dyn FileAccess> {
    #[cfg(desktop)]
    {
        std::sync::Arc::new(DesktopFileAccess(handle.clone()))
    }
    #[cfg(target_os = "android")]
    {
        std::sync::Arc::new(AndroidFileAccess(handle.clone()))
    }
    #[cfg(not(any(desktop, target_os = "android")))]
    {
        let _ = handle;
        std::sync::Arc::new(PathFileAccess)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_refuses_a_concurrent_writer_and_releases_on_close() {
        let root =
            std::env::temp_dir().join(format!("lettuce-export-concurrent-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("root");
        let path = root.join("target");
        let uri = path.to_str().expect("URI");
        let mut first = create_export_path(uri, None, &|_, _| Ok(false)).expect("first writer");
        first.write_all(b"first export").expect("write");
        assert!(matches!(
            create_export_path(uri, None, &|_, _| Ok(false)),
            Err(FileAccessError::Busy)
        ));
        assert_eq!(
            std::fs::read(&path).expect("preserved write"),
            b"first export"
        );
        drop(first);
        let mut retry = create_export_path(uri, None, &|_, _| Ok(false)).expect("released target");
        retry.write_all(b"retry").expect("retry write");
        retry.flush().expect("flush");
        assert_eq!(std::fs::read(&path).expect("export"), b"retry");
        drop(retry);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn export_retries_after_process_exit_during_a_write() {
        let root =
            std::env::temp_dir().join(format!("lettuce-export-crash-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("root");
        let source_path = root.join("source");
        std::fs::write(&source_path, b"complete export contents").expect("source");
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", "files::tests::export_crash_child", "--nocapture"])
            .env("LETTUCE_SLICE12_EXPORT_CRASH_ROOT", &root)
            .status()
            .expect("child");
        assert_eq!(status.code(), Some(77));
        let path = root.join("target");
        assert_eq!(
            std::fs::read(&path).expect("interrupted export"),
            b"partial"
        );
        let mut source = std::fs::File::open(&source_path).expect("source");
        let mut target = create_export_path(path.to_str().expect("URI"), Some(&source), &|_, _| {
            Ok(false)
        })
        .expect("retry after crash");
        std::io::copy(&mut source, &mut target).expect("copy");
        target.flush().expect("flush");
        assert_eq!(
            std::fs::read(&path).expect("complete export"),
            b"complete export contents"
        );
        assert_eq!(
            std::fs::read(&source_path).expect("preserved source"),
            b"complete export contents"
        );
        drop(target);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn export_crash_child() {
        let Some(root) = std::env::var_os("LETTUCE_SLICE12_EXPORT_CRASH_ROOT") else {
            return;
        };
        let path = Path::new(&root).join("target");
        let mut target = create_export_path(path.to_str().expect("URI"), None, &|_, _| Ok(false))
            .expect("writer");
        target.write_all(b"partial").expect("partial write");
        target.flush().expect("flush");
        std::process::exit(77);
    }

    #[test]
    fn export_checks_reserved_locations_before_creation_and_files_before_truncation() {
        let root =
            std::env::temp_dir().join(format!("lettuce-export-protected-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("test operation succeeds");
        let path = root.join("reserved");
        let uri = path.to_str().expect("test operation succeeds");
        assert!(matches!(
            create_export_path(uri, None, &|_, target| Ok(target.is_none())),
            Err(FileAccessError::SourceIsTarget)
        ));
        assert!(!path.exists());
        std::fs::write(&path, b"preserved database").expect("test operation succeeds");
        assert!(matches!(
            create_export_path(uri, None, &|_, target| Ok(target.is_some())),
            Err(FileAccessError::SourceIsTarget)
        ));
        assert_eq!(
            std::fs::read(&path).expect("test operation succeeds"),
            b"preserved database"
        );
        assert!(matches!(
            create_export_path(uri, None, &|_, target| {
                if target.is_some() {
                    Err(FileAccessError::Io)
                } else {
                    Ok(false)
                }
            }),
            Err(FileAccessError::Io)
        ));
        assert_eq!(
            std::fs::read(&path).expect("test operation succeeds"),
            b"preserved database"
        );
        std::fs::remove_dir_all(root).expect("test operation succeeds");
    }

    #[test]
    fn export_refuses_source_and_hard_link_before_truncation() {
        let root = std::env::temp_dir().join(format!("lettuce-export-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("test operation succeeds");
        let source_path = root.join("source");
        let alias_path = root.join("alias");
        std::fs::write(&source_path, b"preserved log").expect("test operation succeeds");
        let _ = std::fs::remove_file(&alias_path);
        std::fs::hard_link(&source_path, &alias_path).expect("test operation succeeds");
        let source = std::fs::File::open(&source_path).expect("test operation succeeds");
        for target in [&source_path, &alias_path] {
            assert!(matches!(
                create_export_path(
                    target.to_str().expect("test operation succeeds"),
                    Some(&source),
                    &|_, _| Ok(false)
                ),
                Err(FileAccessError::SourceIsTarget)
            ));
            assert_eq!(
                std::fs::read(&source_path).expect("test operation succeeds"),
                b"preserved log"
            );
        }
        std::fs::remove_dir_all(root).expect("test operation succeeds");
    }

    #[test]
    fn export_truncates_a_distinct_target() {
        let root =
            std::env::temp_dir().join(format!("lettuce-export-distinct-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("test operation succeeds");
        let source_path = root.join("source");
        let target_path = root.join("target");
        std::fs::write(&source_path, b"short").expect("test operation succeeds");
        std::fs::write(&target_path, b"longer old contents").expect("test operation succeeds");
        let mut source = std::fs::File::open(&source_path).expect("test operation succeeds");
        let mut target = create_export_path(
            target_path.to_str().expect("test operation succeeds"),
            Some(&source),
            &|_, _| Ok(false),
        )
        .expect("test operation succeeds");
        std::io::copy(&mut source, &mut target).expect("test operation succeeds");
        target.flush().expect("test operation succeeds");
        assert_eq!(
            std::fs::read(&target_path).expect("test operation succeeds"),
            b"short"
        );
        std::fs::remove_dir_all(root).expect("test operation succeeds");
    }
}

fn remove_path(uri: &str) -> Result<(), FileAccessError> {
    std::fs::remove_file(uri).map_err(|failure| access_error(&failure))
}

#[cfg(test)]
mod backup_publication_tests {
    use super::*;

    #[test]
    fn missing_backup_classification_preserves_permission_and_volume_failures() {
        assert_eq!(
            classify_backup_metadata_error(false, false),
            FileAccessError::PermissionDenied
        );
        assert_eq!(
            classify_backup_metadata_error(false, true),
            FileAccessError::PermissionDenied
        );
        assert_eq!(
            classify_backup_metadata_error(true, false),
            FileAccessError::Io
        );
        assert_eq!(
            classify_backup_metadata_error(true, true),
            FileAccessError::NotFound
        );
    }

    #[test]
    fn review_backup_paths_resolve_parent_aliases() {
        let root =
            std::env::temp_dir().join(format!("lettuce-backup-paths-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("sub")).expect("folder");
        let root = root.canonicalize().expect("root");
        let target = root.join("backup.lettuce");
        let mut aliases = vec![root.join("sub/../backup.lettuce")];
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&root, root.join("alias")).expect("parent alias");
            aliases.push(root.join("alias/backup.lettuce"));
        }
        for alias in aliases {
            let intent = backup_temporary_location_path(
                Some(alias.to_str().expect("alias")),
                "backup.partial",
            )
            .expect("intent");
            assert_eq!(intent, root.join("backup.partial").display().to_string());
            std::fs::write(&intent, b"complete archive").expect("temporary");
            assert_eq!(
                publish_backup_path(&intent, Some(alias.to_str().expect("alias")), &|_, _| Ok(
                    false
                ))
                .expect("publish"),
                target.display().to_string()
            );
        }
        #[cfg(unix)]
        {
            let referent = root.join("referent.lettuce");
            std::fs::write(&referent, b"kept referent").expect("referent");
            std::fs::remove_file(&target).expect("old target");
            std::os::unix::fs::symlink(&referent, &target).expect("target symlink");
            let intent = backup_temporary_location_path(
                Some(target.to_str().expect("target")),
                "backup.partial",
            )
            .expect("intent");
            std::fs::write(&intent, b"replacement").expect("temporary");
            assert_eq!(
                publish_backup_path(&intent, Some(target.to_str().expect("target")), &|_, _| Ok(
                    false
                ))
                .expect("publish"),
                target.display().to_string()
            );
            assert_eq!(std::fs::read(&referent).expect("kept"), b"kept referent");
            assert!(!target.symlink_metadata().expect("target").is_symlink());
        }
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn backup_publication_replaces_the_path_after_complete_file_sync() {
        let root =
            std::env::temp_dir().join(format!("lettuce-backup-publish-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("folder");
        let target = root.join("backup.lettuce");
        std::fs::write(&target, b"old backup").expect("old backup");
        let intent =
            backup_temporary_location_path(Some(target.to_str().expect("path")), "backup.partial")
                .expect("intent");
        let (uri, mut writer) =
            create_backup_temporary_path(&intent, &|_, _| Ok(false)).expect("temporary");
        writer.write_all(b"complete new backup").expect("write");
        writer.flush().expect("flush");
        drop(writer);
        std::fs::File::open(&uri)
            .expect("temporary")
            .sync_all()
            .expect("durable");
        assert_eq!(
            std::fs::read(&target).expect("old still active"),
            b"old backup"
        );
        assert_eq!(
            publish_backup_path(&uri, Some(target.to_str().expect("path")), &|_, _| Ok(
                false
            ))
            .expect("publish"),
            target.display().to_string()
        );
        assert_eq!(std::fs::read(&target).expect("new"), b"complete new backup");
        assert!(!Path::new(&uri).exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn review_backup_replacement_does_not_require_an_advisory_target_lease() {
        let root = std::env::temp_dir().join(format!(
            "lettuce-backup-target-lease-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("folder");
        let target = root.join("backup.lettuce");
        let temporary = root.join("backup.partial");
        std::fs::write(&target, b"old").expect("old");
        std::fs::write(&temporary, b"new complete backup").expect("new");
        let reader = std::fs::File::open(&target).expect("reader");
        fs4::fs_std::FileExt::lock_shared(&reader).expect("external reader");
        publish_backup_path(
            temporary.to_str().expect("temporary"),
            Some(target.to_str().expect("target")),
            &|_, _| Ok(false),
        )
        .expect("atomic replacement permits readers");
        assert_eq!(
            std::fs::read(&target).expect("new file"),
            b"new complete backup"
        );
        assert!(!temporary.exists());
        drop(reader);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn backup_publication_never_overwrites_a_protected_target() {
        let root =
            std::env::temp_dir().join(format!("lettuce-backup-protected-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("folder");
        let target = root.join("app.db");
        let temporary = root.join("backup.partial");
        std::fs::write(&target, b"user database").expect("database");
        std::fs::write(&temporary, b"backup").expect("backup");
        assert_eq!(
            publish_backup_path(
                temporary.to_str().expect("temp"),
                Some(target.to_str().expect("target")),
                &|_, _| Ok(true)
            ),
            Err(FileAccessError::SourceIsTarget)
        );
        assert_eq!(std::fs::read(&target).expect("kept"), b"user database");
        assert!(temporary.is_file());
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}

#[cfg(any(target_os = "android", test))]
fn classify_backup_metadata_error(permission: bool, volume_available: bool) -> FileAccessError {
    if !permission {
        FileAccessError::PermissionDenied
    } else if !volume_available {
        FileAccessError::Io
    } else {
        FileAccessError::NotFound
    }
}
