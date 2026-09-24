//! The ONNX Runtime shared library every ONNX consumer (embeddings, the
//! companion emotion classifier, Kokoro) loads: the release this binary
//! binds, where a desktop build finds or unpacks it, and the one
//! process-wide initialization all of them go through.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// The ONNX Runtime release loaded at runtime. `ort` =2.0.0-rc.10 with
/// `load-dynamic` binds C API version 22, which only ONNX Runtime 1.22
/// provides, so this version moves together with the `ort` pin.
pub const ONNX_RUNTIME_VERSION: &str = "1.22.0";

const _: () = assert!(
    ort::MINOR_VERSION == 22,
    "ONNX_RUNTIME_VERSION must match the C API version the ort crate binds"
);

const RELEASE_BASE_URL: &str = "https://github.com/microsoft/onnxruntime/releases/download";
const MACOS_PROVIDER_DYLIBS: [&str; 2] = [
    "libonnxruntime_providers_shared.dylib",
    "libonnxruntime_providers_coreml.dylib",
];
const WINDOWS_PROVIDERS_SHARED: &str = "onnxruntime_providers_shared.dll";
const MAX_LINK_HOPS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnnxRuntimeHostOs {
    Windows,
    Linux,
    MacOs,
}

impl OnnxRuntimeHostOs {
    /// The desktop OS this binary runs on; `None` on mobile, where the
    /// runtime ships inside the app and is never downloaded.
    #[must_use]
    pub const fn current() -> Option<Self> {
        if cfg!(any(target_os = "android", target_os = "ios")) {
            None
        } else if cfg!(target_os = "windows") {
            Some(Self::Windows)
        } else if cfg!(target_os = "macos") {
            Some(Self::MacOs)
        } else if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else {
            None
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::MacOs => "macos",
        }
    }
}

/// The file name the runtime library is kept under on `os`.
#[must_use]
pub fn onnx_runtime_library_name(os: OnnxRuntimeHostOs) -> String {
    match os {
        OnnxRuntimeHostOs::Windows => "onnxruntime.dll".to_owned(),
        OnnxRuntimeHostOs::Linux => "libonnxruntime.so".to_owned(),
        OnnxRuntimeHostOs::MacOs => format!("libonnxruntime.{ONNX_RUNTIME_VERSION}.dylib"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnnxRuntimeArchiveFormat {
    Zip,
    TarGz,
}

/// One official release archive that carries the runtime for this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxRuntimeArchive {
    pub url: String,
    pub file_name: String,
    pub format: OnnxRuntimeArchiveFormat,
    /// The archive path of the library, below the archive's `lib` folder.
    pub library_entry: String,
    /// The name the library is saved under in the install folder.
    pub library_name: String,
    /// When set, every file with this extension directly in the `lib`
    /// folder is unpacked (Windows DLLs, macOS provider dylibs); otherwise
    /// only the library is.
    pub sibling_extension: Option<&'static str>,
}

impl OnnxRuntimeArchive {
    fn release(
        package: &str,
        format: OnnxRuntimeArchiveFormat,
        library_file: &str,
        library_name: String,
        sibling_extension: Option<&'static str>,
    ) -> Self {
        let directory = format!("onnxruntime-{package}-{ONNX_RUNTIME_VERSION}");
        let extension = match format {
            OnnxRuntimeArchiveFormat::Zip => "zip",
            OnnxRuntimeArchiveFormat::TarGz => "tgz",
        };
        let file_name = format!("{directory}.{extension}");
        Self {
            url: format!("{RELEASE_BASE_URL}/v{ONNX_RUNTIME_VERSION}/{file_name}"),
            file_name,
            format,
            library_entry: format!("{directory}/lib/{library_file}"),
            library_name,
            sibling_extension,
        }
    }

    fn library_directory(&self) -> &str {
        self.library_entry
            .rsplit_once('/')
            .map_or("", |(directory, _)| directory)
    }
}

/// The release archives to try, in order, for `os` on `arch` (a
/// `std::env::consts::ARCH` value). macOS falls back to the universal2
/// build when the architecture-specific archive fails.
pub fn onnx_runtime_archives(
    os: OnnxRuntimeHostOs,
    arch: &str,
) -> Result<Vec<OnnxRuntimeArchive>, OnnxRuntimeProvisionError> {
    let library_name = onnx_runtime_library_name(os);
    match (os, arch) {
        (OnnxRuntimeHostOs::Windows, "x86_64") => Ok(vec![OnnxRuntimeArchive::release(
            "win-x64",
            OnnxRuntimeArchiveFormat::Zip,
            "onnxruntime.dll",
            library_name,
            Some("dll"),
        )]),
        (OnnxRuntimeHostOs::Linux, "x86_64") => Ok(vec![OnnxRuntimeArchive::release(
            "linux-x64",
            OnnxRuntimeArchiveFormat::TarGz,
            &format!("libonnxruntime.so.{ONNX_RUNTIME_VERSION}"),
            library_name,
            None,
        )]),
        (OnnxRuntimeHostOs::MacOs, "aarch64" | "x86_64") => {
            let package = if arch == "aarch64" {
                "osx-arm64"
            } else {
                "osx-x86_64"
            };
            Ok([package, "osx-universal2"]
                .into_iter()
                .map(|package| {
                    OnnxRuntimeArchive::release(
                        package,
                        OnnxRuntimeArchiveFormat::TarGz,
                        &library_name,
                        library_name.clone(),
                        Some("dylib"),
                    )
                })
                .collect())
        }
        _ => Err(OnnxRuntimeProvisionError::UnsupportedPlatform {
            os: os.as_str().to_owned(),
            arch: arch.to_owned(),
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OnnxRuntimeProvisionError {
    #[error("ONNX Runtime is not available for {os} {arch}")]
    UnsupportedPlatform { os: String, arch: String },
    #[error("the ONNX Runtime archive could not be read: {0}")]
    Archive(String),
    #[error("the ONNX Runtime archive has no {0}")]
    MissingEntry(String),
    #[error("linked ONNX Runtime entry {link} points to missing target {target}")]
    BrokenLink { link: String, target: String },
    #[error("ONNX Runtime archive entry {0} is empty")]
    EmptyEntry(String),
    #[error("ONNX Runtime files could not be written: {0}")]
    Io(String),
    #[error("the unpacked ONNX Runtime library at {0} is not usable")]
    Unusable(PathBuf),
    #[error("unpacking ONNX Runtime was cancelled")]
    Cancelled,
}

/// Where the runtime library came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnnxRuntimeSource {
    /// The `ORT_DYLIB_PATH` override.
    Override,
    /// A library shipped inside the app bundle.
    Bundled,
    /// A release archive unpacked into the install folder.
    Downloaded,
    /// The library this process already committed its ONNX Runtime from.
    Loaded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOnnxRuntime {
    pub library: PathBuf,
    pub source: OnnxRuntimeSource,
}

/// The folders a desktop runtime is looked up in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxRuntimeLocations {
    /// Where downloaded runtimes are unpacked.
    pub install_dir: PathBuf,
    /// The app's bundled resource folder, when the host has one.
    pub resource_dir: Option<PathBuf>,
}

/// A regular file with bytes; a missing, zero-byte or non-file path is not.
#[must_use]
pub fn is_nonempty_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

/// Finds a runtime without downloading: the `ORT_DYLIB_PATH` override, then
/// a bundled library, then an earlier download. A zero-byte or unusable
/// downloaded library is deleted so the next download replaces it; a
/// downloaded macOS runtime is re-signed for library validation.
#[must_use]
pub fn resolve_installed_onnx_runtime(
    os: OnnxRuntimeHostOs,
    override_path: Option<&OsStr>,
    locations: &OnnxRuntimeLocations,
) -> Option<ResolvedOnnxRuntime> {
    if let Some(library) = override_path.and_then(|path| usable_override(os, path)) {
        return Some(ResolvedOnnxRuntime {
            library,
            source: OnnxRuntimeSource::Override,
        });
    }
    if let Some(library) = locations
        .resource_dir
        .as_deref()
        .and_then(|resources| bundled_library(os, resources))
    {
        tracing::info!(path = %library.display(), "using bundled ONNX Runtime");
        warn_missing_macos_providers(os, &library);
        return Some(ResolvedOnnxRuntime {
            library,
            source: OnnxRuntimeSource::Bundled,
        });
    }
    downloaded_library(os, &locations.install_dir).map(|library| ResolvedOnnxRuntime {
        library,
        source: OnnxRuntimeSource::Downloaded,
    })
}

fn usable_override(os: OnnxRuntimeHostOs, value: &OsStr) -> Option<PathBuf> {
    let value = value.to_str().map_or_else(
        || value.to_string_lossy().into_owned(),
        |value| value.trim().to_owned(),
    );
    if value.is_empty() {
        return None;
    }
    let path = PathBuf::from(value);
    if !is_nonempty_file(&path) {
        if std::fs::symlink_metadata(&path).is_ok() {
            tracing::warn!(
                path = %path.display(),
                "ORT_DYLIB_PATH points to an empty or invalid file; ignoring it"
            );
        }
        return None;
    }
    match os {
        OnnxRuntimeHostOs::Windows => windows_providers_present(&path).then_some(path),
        OnnxRuntimeHostOs::MacOs => {
            if library_is_usable(os, &path) {
                warn_missing_macos_providers(os, &path);
                Some(path)
            } else {
                tracing::warn!(
                    path = %path.display(),
                    "ORT_DYLIB_PATH is not a usable macOS MH_DYLIB for this architecture; ignoring it"
                );
                None
            }
        }
        OnnxRuntimeHostOs::Linux => Some(path),
    }
}

fn bundled_candidates(os: OnnxRuntimeHostOs) -> Vec<String> {
    match os {
        OnnxRuntimeHostOs::Windows => vec![
            "onnxruntime/onnxruntime.dll".to_owned(),
            "onnxruntime.dll".to_owned(),
        ],
        OnnxRuntimeHostOs::MacOs => {
            let versioned = onnx_runtime_library_name(os);
            vec![
                format!("onnxruntime/{versioned}"),
                versioned,
                "onnxruntime/libonnxruntime.dylib".to_owned(),
                "libonnxruntime.dylib".to_owned(),
            ]
        }
        OnnxRuntimeHostOs::Linux => vec![
            "onnxruntime/libonnxruntime.so".to_owned(),
            "libonnxruntime.so".to_owned(),
        ],
    }
}

fn bundled_library(os: OnnxRuntimeHostOs, resources: &Path) -> Option<PathBuf> {
    bundled_candidates(os)
        .into_iter()
        .map(|candidate| resources.join(candidate))
        .find(|path| library_is_usable(os, path))
}

fn downloaded_library(os: OnnxRuntimeHostOs, install_dir: &Path) -> Option<PathBuf> {
    let library = install_dir.join(onnx_runtime_library_name(os));
    if !is_nonempty_file(&library) {
        if std::fs::symlink_metadata(&library).is_ok() {
            tracing::warn!(
                path = %library.display(),
                "deleting an empty ONNX Runtime library so it is downloaded again"
            );
            remove_quietly(&library);
        }
        return None;
    }
    match os {
        OnnxRuntimeHostOs::Windows => windows_providers_present(&library).then_some(library),
        OnnxRuntimeHostOs::MacOs => {
            if library_is_usable(os, &library) {
                sign_downloaded_macos_dylibs(install_dir);
                warn_missing_macos_providers(os, &library);
                Some(library)
            } else {
                tracing::warn!(
                    path = %library.display(),
                    "deleting an unusable ONNX Runtime library so it is downloaded again"
                );
                remove_quietly(&library);
                None
            }
        }
        OnnxRuntimeHostOs::Linux => Some(library),
    }
}

fn windows_providers_present(library: &Path) -> bool {
    library
        .parent()
        .is_some_and(|directory| is_nonempty_file(&directory.join(WINDOWS_PROVIDERS_SHARED)))
}

fn remove_quietly(path: &Path) {
    if let Err(error) = std::fs::remove_file(path) {
        tracing::warn!(path = %path.display(), %error, "could not delete an ONNX Runtime file");
    }
}

fn library_is_usable(os: OnnxRuntimeHostOs, path: &Path) -> bool {
    is_nonempty_file(path) && (os != OnnxRuntimeHostOs::MacOs || macos::dylib_is_usable(path))
}

fn warn_missing_macos_providers(os: OnnxRuntimeHostOs, library: &Path) {
    if os != OnnxRuntimeHostOs::MacOs {
        return;
    }
    let Some(directory) = library.parent() else {
        return;
    };
    for provider in MACOS_PROVIDER_DYLIBS {
        if !is_nonempty_file(&directory.join(provider)) {
            tracing::warn!(
                library = %library.display(),
                provider,
                "ONNX Runtime provider dylib is missing; CoreML acceleration may be unavailable"
            );
        }
    }
}

/// Unpacks a downloaded release archive into `install_dir`, checks the
/// library (on Windows together with its shared provider library) and, on
/// macOS, re-signs it; returns the runtime to load. `cancelled` is polled
/// between archive entries.
pub fn install_onnx_runtime_archive(
    os: OnnxRuntimeHostOs,
    archive: &OnnxRuntimeArchive,
    archive_file: &Path,
    install_dir: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<ResolvedOnnxRuntime, OnnxRuntimeProvisionError> {
    let library = extract_onnx_runtime_archive(archive, archive_file, install_dir, cancelled)?;
    if !library_is_usable(os, &library) {
        remove_quietly(&library);
        return Err(OnnxRuntimeProvisionError::Unusable(library));
    }
    if os == OnnxRuntimeHostOs::Windows && !windows_providers_present(&library) {
        return Err(OnnxRuntimeProvisionError::MissingEntry(
            WINDOWS_PROVIDERS_SHARED.to_owned(),
        ));
    }
    sign_downloaded_macos_dylibs(install_dir);
    warn_missing_macos_providers(os, &library);
    Ok(ResolvedOnnxRuntime {
        library,
        source: OnnxRuntimeSource::Downloaded,
    })
}

#[derive(Debug)]
enum ArchiveEntry {
    File(Vec<u8>),
    Link(String),
}

/// Unpacks the library (and, per [`OnnxRuntimeArchive::sibling_extension`],
/// its sibling libraries) into `install_dir`. Symbolic and hard links are
/// written as full copies of their target, and every written file must be
/// non-empty; files land under a temporary name first so a failed write
/// never leaves a library that looks installed. A sibling whose link cannot
/// be resolved, or that is empty, is skipped; only the library itself is
/// required, and it is written last, so an interrupted unpack never leaves a
/// usable library without its siblings. Only entries the install keeps are
/// read into memory. `cancelled` is polled between entries.
pub fn extract_onnx_runtime_archive(
    archive: &OnnxRuntimeArchive,
    archive_file: &Path,
    install_dir: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<PathBuf, OnnxRuntimeProvisionError> {
    let entries = match archive.format {
        OnnxRuntimeArchiveFormat::Zip => zip_entries(archive, archive_file, cancelled)?,
        OnnxRuntimeArchiveFormat::TarGz => tar_entries(archive, archive_file, cancelled)?,
    };
    std::fs::create_dir_all(install_dir).map_err(io_error)?;
    let library_file = archive
        .library_entry
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    let mut outputs = Vec::new();
    if let Some(extension) = archive.sibling_extension {
        let mut names = entries
            .keys()
            .filter(|name| Path::new(name).extension() == Some(OsStr::new(extension)))
            .cloned()
            .collect::<Vec<_>>();
        names.sort();
        for name in names {
            outputs.push((name.clone(), name));
        }
    } else if entries.contains_key(&library_file) {
        outputs.push((library_file.clone(), archive.library_name.clone()));
    }
    if !outputs
        .iter()
        .any(|(_, output)| *output == archive.library_name)
    {
        return Err(OnnxRuntimeProvisionError::MissingEntry(
            archive.library_entry.clone(),
        ));
    }
    outputs.sort_by_key(|(_, output)| *output == archive.library_name);
    for (name, output) in &outputs {
        check_cancelled(cancelled)?;
        let bytes = match resolve_entry(&entries, name) {
            Ok(bytes) => bytes,
            Err(error) if *output != archive.library_name => {
                tracing::warn!(entry = %name, %error, "skipping an unusable ONNX Runtime archive entry");
                continue;
            }
            Err(error) => return Err(error),
        };
        write_nonempty(&install_dir.join(output), bytes)?;
    }
    let library = install_dir.join(&archive.library_name);
    if is_nonempty_file(&library) {
        Ok(library)
    } else {
        Err(OnnxRuntimeProvisionError::MissingEntry(
            archive.library_entry.clone(),
        ))
    }
}

fn resolve_entry<'a>(
    entries: &'a HashMap<String, ArchiveEntry>,
    name: &str,
) -> Result<&'a [u8], OnnxRuntimeProvisionError> {
    let mut current = name;
    for _ in 0..=MAX_LINK_HOPS {
        match entries.get(current) {
            Some(ArchiveEntry::File(bytes)) if bytes.is_empty() => {
                return Err(OnnxRuntimeProvisionError::EmptyEntry(current.to_owned()));
            }
            Some(ArchiveEntry::File(bytes)) => return Ok(bytes),
            Some(ArchiveEntry::Link(target)) => current = target,
            None => {
                return Err(OnnxRuntimeProvisionError::BrokenLink {
                    link: name.to_owned(),
                    target: current.to_owned(),
                });
            }
        }
    }
    Err(OnnxRuntimeProvisionError::BrokenLink {
        link: name.to_owned(),
        target: current.to_owned(),
    })
}

fn write_nonempty(path: &Path, bytes: &[u8]) -> Result<(), OnnxRuntimeProvisionError> {
    let name = path
        .file_name()
        .ok_or_else(|| OnnxRuntimeProvisionError::Io(path.display().to_string()))?;
    if bytes.is_empty() {
        return Err(OnnxRuntimeProvisionError::EmptyEntry(
            name.to_string_lossy().into_owned(),
        ));
    }
    let mut partial_name = OsStr::new(".").to_os_string();
    partial_name.push(name);
    partial_name.push(".partial");
    let partial = path.with_file_name(partial_name);
    if let Err(error) = std::fs::write(&partial, bytes) {
        if std::fs::symlink_metadata(&partial).is_ok() {
            remove_quietly(&partial);
        }
        return Err(io_error(error));
    }
    if let Err(error) = std::fs::rename(&partial, path) {
        remove_quietly(&partial);
        return Err(io_error(error));
    }
    if is_nonempty_file(path) {
        Ok(())
    } else {
        Err(OnnxRuntimeProvisionError::EmptyEntry(
            name.to_string_lossy().into_owned(),
        ))
    }
}

fn check_cancelled(cancelled: &dyn Fn() -> bool) -> Result<(), OnnxRuntimeProvisionError> {
    if cancelled() {
        Err(OnnxRuntimeProvisionError::Cancelled)
    } else {
        Ok(())
    }
}

fn io_error(error: std::io::Error) -> OnnxRuntimeProvisionError {
    OnnxRuntimeProvisionError::Io(error.to_string())
}

fn archive_error(error: impl std::fmt::Display) -> OnnxRuntimeProvisionError {
    OnnxRuntimeProvisionError::Archive(error.to_string())
}

/// `path` with `.` and `..` folded away, `/`-separated; `None` when it
/// escapes the archive root.
fn normalize_archive_path(path: &Path) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(parts.join("/"))
}

/// The name of `path` when it sits directly in the archive's `lib` folder.
fn library_directory_name(archive: &OnnxRuntimeArchive, path: &str) -> Option<String> {
    let (directory, name) = path.rsplit_once('/')?;
    (directory == archive.library_directory() && !name.is_empty()).then(|| name.to_owned())
}

/// A link's target as a name in the `lib` folder. Symbolic link targets are
/// relative to the link; hard link targets are archive paths.
fn link_target_name(
    archive: &OnnxRuntimeArchive,
    target: &Path,
    relative_to_link: bool,
) -> Option<String> {
    let joined = if relative_to_link {
        Path::new(archive.library_directory()).join(target)
    } else {
        target.to_path_buf()
    };
    normalize_archive_path(&joined).and_then(|path| library_directory_name(archive, &path))
}

/// Whether an entry of the `lib` folder is kept: a sibling with the
/// archive's extension, or else a name of the library's link family (such
/// as `libonnxruntime.so.1`).
fn wanted_entry(archive: &OnnxRuntimeArchive, name: &str) -> bool {
    match archive.sibling_extension {
        Some(extension) => Path::new(name).extension() == Some(OsStr::new(extension)),
        None => name.starts_with(&archive.library_name),
    }
}

fn tar_entries(
    archive: &OnnxRuntimeArchive,
    archive_file: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<HashMap<String, ArchiveEntry>, OnnxRuntimeProvisionError> {
    let file = std::fs::File::open(archive_file).map_err(io_error)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut entries = HashMap::new();
    for entry in tar.entries().map_err(archive_error)? {
        check_cancelled(cancelled)?;
        let mut entry = entry.map_err(archive_error)?;
        let Some(path) = entry
            .path()
            .ok()
            .and_then(|path| normalize_archive_path(&path))
        else {
            continue;
        };
        let Some(name) =
            library_directory_name(archive, &path).filter(|name| wanted_entry(archive, name))
        else {
            continue;
        };
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            let target = entry
                .link_name()
                .ok()
                .flatten()
                .and_then(|target| link_target_name(archive, &target, kind.is_symlink()));
            match target {
                Some(target) => {
                    entries.insert(name, ArchiveEntry::Link(target));
                }
                None => {
                    tracing::debug!(entry = %path, "ignoring an ONNX Runtime archive link outside lib")
                }
            }
        } else if kind.is_file() {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(archive_error)?;
            entries.insert(name, ArchiveEntry::File(bytes));
        }
    }
    Ok(entries)
}

fn zip_entries(
    archive: &OnnxRuntimeArchive,
    archive_file: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<HashMap<String, ArchiveEntry>, OnnxRuntimeProvisionError> {
    let file = std::fs::File::open(archive_file).map_err(io_error)?;
    let mut zip = zip::ZipArchive::new(file).map_err(archive_error)?;
    let mut entries = HashMap::new();
    for index in 0..zip.len() {
        check_cancelled(cancelled)?;
        let mut entry = zip.by_index(index).map_err(archive_error)?;
        let Some(path) = normalize_archive_path(Path::new(entry.name())) else {
            continue;
        };
        let Some(name) =
            library_directory_name(archive, &path).filter(|name| wanted_entry(archive, name))
        else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(archive_error)?;
        if entry.is_symlink() {
            match String::from_utf8(bytes)
                .ok()
                .and_then(|target| link_target_name(archive, Path::new(&target), true))
            {
                Some(target) => {
                    entries.insert(name, ArchiveEntry::Link(target));
                }
                None => {
                    tracing::debug!(entry = %path, "ignoring an ONNX Runtime archive link outside lib")
                }
            }
        } else {
            entries.insert(name, ArchiveEntry::File(bytes));
        }
    }
    Ok(entries)
}

/// What `codesign -d --verbose=2` reported for one dylib.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodesignDisplay {
    pub success: bool,
    pub stderr: String,
}

/// Whether a downloaded dylib must be re-signed ad hoc before the hardened
/// app may load it: when it is unsigned (the display fails) or signed by a
/// real team, whose identity library validation rejects. When `codesign`
/// could not run at all, nothing is changed.
#[must_use]
pub fn macos_dylib_needs_adhoc_signature(display: Option<&CodesignDisplay>) -> bool {
    let Some(display) = display else {
        return false;
    };
    if !display.success {
        return true;
    }
    display.stderr.lines().any(|line| {
        line.strip_prefix("TeamIdentifier=").is_some_and(|team| {
            let team = team.trim();
            !team.is_empty() && team != "not set"
        })
    })
}

fn sign_downloaded_macos_dylibs(install_dir: &Path) {
    macos::sign_downloaded_dylibs(install_dir);
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::Path;
    use std::process::Command;

    use super::{CodesignDisplay, macos_dylib_needs_adhoc_signature};

    fn current_arch() -> Option<&'static str> {
        match std::env::consts::ARCH {
            "aarch64" => Some("arm64"),
            "x86_64" => Some("x86_64"),
            _ => None,
        }
    }

    fn has_arch(path: &Path, expected: &str) -> bool {
        let Ok(output) = Command::new("lipo").arg("-archs").arg(path).output() else {
            return true;
        };
        if !output.status.success() {
            return true;
        }
        String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .any(|arch| arch == expected)
    }

    fn is_dylib(path: &Path) -> bool {
        let Ok(output) = Command::new("otool").arg("-hV").arg(path).output() else {
            return true;
        };
        if !output.status.success() {
            return true;
        }
        String::from_utf8_lossy(&output.stdout).contains("MH_DYLIB")
    }

    pub(super) fn dylib_is_usable(path: &Path) -> bool {
        current_arch().is_none_or(|arch| has_arch(path, arch)) && is_dylib(path)
    }

    fn codesign_display(path: &Path) -> Option<CodesignDisplay> {
        Command::new("codesign")
            .arg("-d")
            .arg("--verbose=2")
            .arg(path)
            .output()
            .ok()
            .map(|output| CodesignDisplay {
                success: output.status.success(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
    }

    pub(super) fn sign_downloaded_dylibs(install_dir: &Path) {
        let Ok(entries) = std::fs::read_dir(install_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("dylib")
                || !macos_dylib_needs_adhoc_signature(codesign_display(&path).as_ref())
            {
                continue;
            }
            let _ = Command::new("xattr")
                .arg("-d")
                .arg("com.apple.quarantine")
                .arg(&path)
                .output();
            match Command::new("codesign")
                .arg("--force")
                .arg("--sign")
                .arg("-")
                .arg(&path)
                .output()
            {
                Ok(output) if output.status.success() => tracing::info!(
                    path = %path.display(),
                    "re-signed ONNX Runtime dylib ad hoc for library validation"
                ),
                Ok(output) => tracing::warn!(
                    path = %path.display(),
                    stderr = %String::from_utf8_lossy(&output.stderr).trim(),
                    "could not re-sign ONNX Runtime dylib ad hoc"
                ),
                Err(error) => tracing::warn!(
                    path = %path.display(),
                    %error,
                    "could not run codesign for ONNX Runtime dylib"
                ),
            }
        }
    }

    pub(super) fn preload_provider_dylibs(directory: &Path) {
        for name in super::MACOS_PROVIDER_DYLIBS {
            let path = directory.join(name);
            if super::is_nonempty_file(&path) {
                let _ = ort::util::preload_dylib(&path);
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod macos {
    use std::path::Path;

    pub(super) fn dylib_is_usable(_path: &Path) -> bool {
        true
    }

    pub(super) fn sign_downloaded_dylibs(_install_dir: &Path) {}

    #[cfg(not(target_os = "ios"))]
    pub(super) fn preload_provider_dylibs(_directory: &Path) {}
}

/// How this process reaches ONNX Runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnnxRuntimeBinding<'a> {
    /// A shared library loaded from this path.
    Library(&'a Path),
    /// The runtime shipped with the app (Android `jniLibs`, iOS).
    Linked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnnxRuntimeInitialization {
    Initialized,
    AlreadyInitialized,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OnnxRuntimeInitError {
    #[error("the ONNX Runtime library path is not valid UTF-8")]
    InvalidPath,
    #[error("the ONNX Runtime library at {0} is missing or empty")]
    MissingLibrary(PathBuf),
    #[error("the ONNX Runtime library could not be loaded: {0}")]
    Load(String),
    #[error("ONNX Runtime initialization panicked: {0}")]
    Panicked(String),
    #[error("ONNX Runtime could not be initialized: {0}")]
    Environment(String),
}

/// The runtime this process committed its ONNX Runtime environment from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommittedOnnxRuntime {
    Library(PathBuf),
    Linked,
}

impl From<OnnxRuntimeBinding<'_>> for CommittedOnnxRuntime {
    fn from(binding: OnnxRuntimeBinding<'_>) -> Self {
        match binding {
            OnnxRuntimeBinding::Library(path) => Self::Library(path.to_path_buf()),
            OnnxRuntimeBinding::Linked => Self::Linked,
        }
    }
}

impl OnnxRuntimeInitError {
    /// Whether the failure happened after `ort` took the library path and
    /// ran its one-shot environment setup. `ort` keeps that path, and its
    /// `OnceLock` counts a failed setup as done, so no later attempt in this
    /// process can succeed safely.
    #[must_use]
    pub const fn is_permanent(&self) -> bool {
        matches!(self, Self::Panicked(_) | Self::Environment(_))
    }
}

#[derive(Debug)]
enum ProcessState {
    Uninitialized,
    Committed(CommittedOnnxRuntime),
    Failed(OnnxRuntimeInitError),
}

#[derive(Debug)]
struct RuntimeOnce {
    state: Mutex<ProcessState>,
}

impl RuntimeOnce {
    const fn new() -> Self {
        Self {
            state: Mutex::new(ProcessState::Uninitialized),
        }
    }

    fn committed(&self) -> Option<CommittedOnnxRuntime> {
        match &*self.state.lock().unwrap_or_else(PoisonError::into_inner) {
            ProcessState::Committed(committed) => Some(committed.clone()),
            ProcessState::Uninitialized | ProcessState::Failed(_) => None,
        }
    }

    /// Runs `commit` until one attempt succeeds or fails permanently.
    /// Failures before `ort` is entered (a missing, unreadable or
    /// unloadable library) leave the process uninitialized so a later call
    /// may retry; a permanent failure is returned to every later call. Once
    /// one commit succeeded, every later binding reuses it, as ONNX Runtime
    /// allows one environment per process.
    fn initialize(
        &self,
        binding: OnnxRuntimeBinding<'_>,
        commit: impl FnOnce() -> Result<(), OnnxRuntimeInitError>,
    ) -> Result<OnnxRuntimeInitialization, OnnxRuntimeInitError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        match &*state {
            ProcessState::Committed(existing) => {
                let requested = CommittedOnnxRuntime::from(binding);
                if *existing != requested {
                    tracing::warn!(
                        ?existing,
                        ?requested,
                        "ONNX Runtime is already initialized from another library; continuing with it"
                    );
                }
                return Ok(OnnxRuntimeInitialization::AlreadyInitialized);
            }
            ProcessState::Failed(error) => return Err(error.clone()),
            ProcessState::Uninitialized => {}
        }
        match commit() {
            Ok(()) => {
                *state = ProcessState::Committed(binding.into());
                Ok(OnnxRuntimeInitialization::Initialized)
            }
            Err(error) => {
                if error.is_permanent() {
                    *state = ProcessState::Failed(error.clone());
                }
                Err(error)
            }
        }
    }
}

static PROCESS_RUNTIME: RuntimeOnce = RuntimeOnce::new();

/// The runtime this process's ONNX Runtime environment was committed from,
/// once [`initialize_process_onnx_runtime`] succeeded.
#[must_use]
pub fn committed_onnx_runtime() -> Option<CommittedOnnxRuntime> {
    PROCESS_RUNTIME.committed()
}

/// Initializes ONNX Runtime for the whole process, once. The library is
/// preloaded first, so a missing or broken file fails with a retryable
/// error before `ort` pins its path; the `ort` environment is committed
/// inside `catch_unwind` because `ort` panics when loading fails, and a
/// failure from that point on is permanent for the process (see
/// [`OnnxRuntimeInitError::is_permanent`]). `name` labels the environment
/// in ONNX Runtime's logs; the first successful caller's name is kept.
pub fn initialize_process_onnx_runtime(
    binding: OnnxRuntimeBinding<'_>,
    name: &str,
) -> Result<OnnxRuntimeInitialization, OnnxRuntimeInitError> {
    PROCESS_RUNTIME.initialize(binding, || commit_environment(binding, name))
}

fn commit_environment(
    binding: OnnxRuntimeBinding<'_>,
    name: &str,
) -> Result<(), OnnxRuntimeInitError> {
    let library = match binding {
        OnnxRuntimeBinding::Library(path) => {
            let text = path.to_str().ok_or(OnnxRuntimeInitError::InvalidPath)?;
            if !is_nonempty_file(path) {
                return Err(OnnxRuntimeInitError::MissingLibrary(path.to_path_buf()));
            }
            preload_library(path)?;
            Some(text.to_owned())
        }
        OnnxRuntimeBinding::Linked => {
            preload_linked_library()?;
            None
        }
    };
    let committed = std::panic::catch_unwind(|| match library {
        #[cfg(not(target_os = "ios"))]
        Some(library) => ort::init_from(library).with_name(name).commit(),
        _ => ort::init().with_name(name).commit(),
    })
    .map_err(|payload| OnnxRuntimeInitError::Panicked(panic_message(payload.as_ref())))?;
    committed
        .map(|_| ())
        .map_err(|error| OnnxRuntimeInitError::Environment(error.to_string()))
}

#[cfg(not(target_os = "ios"))]
fn preload_library(path: &Path) -> Result<(), OnnxRuntimeInitError> {
    if cfg!(target_os = "macos")
        && let Some(directory) = path.parent()
    {
        macos::preload_provider_dylibs(directory);
    }
    ort::util::preload_dylib(path).map_err(|error| OnnxRuntimeInitError::Load(error.to_string()))
}

#[cfg(target_os = "ios")]
fn preload_library(_path: &Path) -> Result<(), OnnxRuntimeInitError> {
    Err(OnnxRuntimeInitError::Load(
        "iOS links ONNX Runtime statically".to_owned(),
    ))
}

#[cfg(target_os = "android")]
fn preload_linked_library() -> Result<(), OnnxRuntimeInitError> {
    ort::util::preload_dylib("libonnxruntime.so")
        .map_err(|error| OnnxRuntimeInitError::Load(error.to_string()))
}

#[cfg(not(target_os = "android"))]
fn preload_linked_library() -> Result<(), OnnxRuntimeInitError> {
    Ok(())
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_owned())
}

#[cfg(test)]
#[path = "ort_runtime_tests.rs"]
mod tests;
