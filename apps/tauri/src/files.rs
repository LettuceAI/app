use std::{io::Write, path::Path};

use lettuce_app::api::{FileAccess, FileAccessError, FileDescription, FileReader, PickFilter};
use tauri::{AppHandle, Runtime};

fn access_error(error: &std::io::Error) -> FileAccessError {
    match error.kind() {
        std::io::ErrorKind::NotFound => FileAccessError::NotFound,
        std::io::ErrorKind::PermissionDenied => FileAccessError::PermissionDenied,
        _ => FileAccessError::Io,
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
    fn describe(&self, uri: &str) -> Result<FileDescription, FileAccessError> {
        describe_path(uri)
    }

    fn open(&self, uri: &str) -> Result<Box<dyn FileReader>, FileAccessError> {
        open_path(uri)
    }

    fn create(&self, uri: &str) -> Result<Box<dyn Write + Send>, FileAccessError> {
        create_path(uri)
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
}

#[cfg(target_os = "android")]
fn platform_error(_error: tauri_plugin_android_fs::Error) -> FileAccessError {
    FileAccessError::Io
}

#[cfg(target_os = "android")]
fn platform_uri(uri: &str) -> Option<tauri_plugin_android_fs::FsUri> {
    uri.contains("://")
        .then(|| tauri_plugin_android_fs::FsUri::from_uri(uri))
}

#[cfg(target_os = "android")]
impl<R: Runtime> FileAccess for AndroidFileAccess<R> {
    fn describe(&self, uri: &str) -> Result<FileDescription, FileAccessError> {
        let Some(uri) = platform_uri(uri) else {
            return describe_path(uri);
        };
        Ok(FileDescription {
            name: self.fs().get_name(&uri).map_err(platform_error)?,
            size: self.fs().get_len(&uri).map_err(platform_error)?,
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
    fn describe(&self, uri: &str) -> Result<FileDescription, FileAccessError> {
        describe_path(uri)
    }

    fn open(&self, uri: &str) -> Result<Box<dyn FileReader>, FileAccessError> {
        open_path(uri)
    }

    fn create(&self, uri: &str) -> Result<Box<dyn Write + Send>, FileAccessError> {
        create_path(uri)
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
