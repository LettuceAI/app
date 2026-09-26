use std::{io::Write, path::Path};

use lettuce_app::api::{FileAccess, FileAccessError, FileDescription, FileReader};

/// Files the desktop dialogs return: filesystem paths, opened with `std::fs`.
pub(crate) struct DesktopFileAccess;

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

impl FileAccess for DesktopFileAccess {
    fn describe(&self, uri: &str) -> Result<FileDescription, FileAccessError> {
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

    fn open(&self, uri: &str) -> Result<Box<dyn FileReader>, FileAccessError> {
        std::fs::File::open(path(uri)?)
            .map(|file| Box::new(file) as Box<dyn FileReader>)
            .map_err(|error| access_error(&error))
    }

    fn create(&self, uri: &str) -> Result<Box<dyn Write + Send>, FileAccessError> {
        std::fs::File::create(path(uri)?)
            .map(|file| Box::new(file) as Box<dyn Write + Send>)
            .map_err(|error| access_error(&error))
    }
}
