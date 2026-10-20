use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

pub const ESPEAK_ANDROID_BUNDLE_REPOSITORY: &str = "LettuceAI/app";
pub const ESPEAK_ANDROID_BUNDLE_TAG: &str = "espeak-android-bundle-v2";
pub const ESPEAK_ANDROID_BUNDLE_ASSET: &str = "kokoro-espeak-android-bundle.tar.gz";
pub const ESPEAK_ANDROID_BUNDLE_SHA256: &str =
    "98b872689263c6b8328da9919874820d61e43238684d8e63129d2f73923345aa";
pub const ESPEAK_ANDROID_BUNDLE_BYTES: u64 = 10_720_044;
/// The eSpeak NG data folder inside the bundle and inside the Kokoro root.
pub const ESPEAK_DATA_DIRECTORY: &str = "espeak-ng-data";

const STAGE_DIRECTORY: &str = "espeak-ng-data.stage";
const MAX_EXTRACTED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;

#[must_use]
pub fn espeak_android_bundle_url() -> String {
    format!(
        "https://github.com/{ESPEAK_ANDROID_BUNDLE_REPOSITORY}/releases/download/{ESPEAK_ANDROID_BUNDLE_TAG}/{ESPEAK_ANDROID_BUNDLE_ASSET}"
    )
}

/// Where Kokoro runs: Android phonemizes with the eSpeak NG library, whose
/// data is installed with the model; desktop runs the `espeak-ng` program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KokoroHostPlatform {
    Desktop,
    Android,
}

impl KokoroHostPlatform {
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "android") {
            Self::Android
        } else {
            Self::Desktop
        }
    }

    #[must_use]
    pub const fn installs_espeak_data(self) -> bool {
        matches!(self, Self::Android)
    }
}

/// Whether an installed Kokoro model can speak on `platform`.
#[must_use]
pub const fn kokoro_model_ready(
    platform: KokoroHostPlatform,
    model_installed: bool,
    espeak_data_installed: bool,
) -> bool {
    model_installed && (!platform.installs_espeak_data() || espeak_data_installed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EspeakBundleError {
    #[error("the eSpeak NG bundle is not a valid archive")]
    InvalidArchive,
    #[error("the eSpeak NG data could not be written")]
    Io,
}

/// Whether `root` holds an extracted `espeak-ng-data` folder.
#[must_use]
pub fn espeak_data_installed(root: &Path) -> bool {
    fs::symlink_metadata(root.join(ESPEAK_DATA_DIRECTORY)).is_ok_and(|metadata| metadata.is_dir())
}

/// Extracts the bundle's `espeak-ng-data` folder into `root`. It is
/// unpacked into a stage folder first and renamed into place, so an
/// interrupted extraction never leaves a folder that looks installed. Any
/// entry with an absolute path, a `..` component, a link or a special file
/// rejects the whole archive; entries outside `espeak-ng-data` are skipped.
pub fn install_espeak_data(archive: &Path, root: &Path) -> Result<PathBuf, EspeakBundleError> {
    let target = root.join(ESPEAK_DATA_DIRECTORY);
    if espeak_data_installed(root) {
        return Ok(target);
    }
    let stage = root.join(STAGE_DIRECTORY);
    remove_directory(&stage)?;
    let result = extract_into(archive, &stage).and_then(|()| {
        fs::rename(stage.join(ESPEAK_DATA_DIRECTORY), &target).map_err(|_| EspeakBundleError::Io)
    });
    let cleanup = remove_directory(&stage);
    result?;
    cleanup?;
    Ok(target)
}

/// Removes the extracted data from `root`: renamed out of place first, so a
/// half-deleted folder never looks installed.
pub fn remove_espeak_data(root: &Path) -> Result<bool, EspeakBundleError> {
    let stage = root.join(STAGE_DIRECTORY);
    remove_directory(&stage)?;
    if !espeak_data_installed(root) {
        return Ok(false);
    }
    fs::rename(root.join(ESPEAK_DATA_DIRECTORY), &stage).map_err(|_| EspeakBundleError::Io)?;
    remove_directory(&stage)?;
    Ok(true)
}

fn remove_directory(path: &Path) -> Result<(), EspeakBundleError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(EspeakBundleError::Io),
    }
}

fn extract_into(archive: &Path, stage: &Path) -> Result<(), EspeakBundleError> {
    fs::create_dir_all(stage).map_err(|_| EspeakBundleError::Io)?;
    let file = fs::File::open(archive).map_err(|_| EspeakBundleError::Io)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut extracted = 0_u64;
    let mut found = false;
    for (index, entry) in tar
        .entries()
        .map_err(|_| EspeakBundleError::InvalidArchive)?
        .enumerate()
    {
        if index >= MAX_ENTRIES {
            return Err(EspeakBundleError::InvalidArchive);
        }
        let mut entry = entry.map_err(|_| EspeakBundleError::InvalidArchive)?;
        let kind = entry.header().entry_type();
        if kind.is_pax_global_extensions()
            || kind.is_pax_local_extensions()
            || kind.is_gnu_longname()
            || kind.is_gnu_longlink()
        {
            continue;
        }
        let path = entry
            .path()
            .map_err(|_| EspeakBundleError::InvalidArchive)?
            .into_owned();
        let relative = archive_relative_path(&path)?;
        if !(kind.is_file() || kind.is_dir()) {
            return Err(EspeakBundleError::InvalidArchive);
        }
        if relative.components().next() != Some(Component::Normal(ESPEAK_DATA_DIRECTORY.as_ref())) {
            continue;
        }
        found = true;
        let destination = stage.join(&relative);
        if kind.is_dir() {
            fs::create_dir_all(&destination).map_err(|_| EspeakBundleError::Io)?;
            continue;
        }
        let size = entry
            .header()
            .size()
            .map_err(|_| EspeakBundleError::InvalidArchive)?;
        extracted = extracted
            .checked_add(size)
            .filter(|total| *total <= MAX_EXTRACTED_BYTES)
            .ok_or(EspeakBundleError::InvalidArchive)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|_| EspeakBundleError::Io)?;
        }
        let mut output = fs::File::create_new(&destination).map_err(|_| EspeakBundleError::Io)?;
        let written = io::copy(&mut (&mut entry).take(size), &mut output)
            .map_err(|_| EspeakBundleError::InvalidArchive)?;
        if written != size {
            return Err(EspeakBundleError::InvalidArchive);
        }
        output.sync_all().map_err(|_| EspeakBundleError::Io)?;
    }
    if found {
        Ok(())
    } else {
        Err(EspeakBundleError::InvalidArchive)
    }
}

fn archive_relative_path(path: &Path) -> Result<PathBuf, EspeakBundleError> {
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(EspeakBundleError::InvalidArchive);
            }
        }
    }
    Ok(relative)
}

#[cfg(test)]
mod tests {
    use lettuce_types::OperationId;

    use super::*;

    enum Entry<'a> {
        File(&'a str, &'a [u8]),
        Directory(&'a str),
        Symlink(&'a str, &'a str),
    }

    fn archive(root: &Path, entries: &[Entry<'_>]) -> PathBuf {
        let path = root.join("bundle.tar.gz");
        let file = fs::File::create(&path).expect("archive");
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            file,
            flate2::Compression::fast(),
        ));
        for entry in entries {
            let mut header = tar::Header::new_gnu();
            match entry {
                Entry::File(name, bytes) => {
                    header.set_entry_type(tar::EntryType::Regular);
                    header.set_size(bytes.len() as u64);
                    header.set_mode(0o644);
                    set_raw_path(&mut header, name);
                    header.set_cksum();
                    builder.append(&header, *bytes).expect("file entry");
                }
                Entry::Directory(name) => {
                    header.set_entry_type(tar::EntryType::Directory);
                    header.set_size(0);
                    header.set_mode(0o755);
                    set_raw_path(&mut header, name);
                    header.set_cksum();
                    builder
                        .append(&header, io::empty())
                        .expect("directory entry");
                }
                Entry::Symlink(name, target) => {
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_size(0);
                    header.set_mode(0o777);
                    set_raw_path(&mut header, name);
                    header.set_link_name(target).expect("link target");
                    header.set_cksum();
                    builder.append(&header, io::empty()).expect("symlink entry");
                }
            }
        }
        builder
            .into_inner()
            .expect("archive body")
            .finish()
            .expect("gzip");
        path
    }

    fn set_raw_path(header: &mut tar::Header, name: &str) {
        let bytes = name.as_bytes();
        let field = &mut header.as_old_mut().name;
        field.fill(0);
        field[..bytes.len()].copy_from_slice(bytes);
    }

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("{name}-{}", OperationId::new()));
        fs::create_dir_all(&root).expect("root");
        root
    }

    #[test]
    fn extracts_only_the_data_folder_through_a_stage() {
        let root = temp_root("espeak-extract");
        let archive = archive(
            &root,
            &[
                Entry::Directory("jniLibs/"),
                Entry::File("jniLibs/arm64-v8a/libttsespeak.so", b"library"),
                Entry::Directory("espeak-ng-data/"),
                Entry::File("espeak-ng-data/en_dict", b"dictionary"),
                Entry::File("./espeak-ng-data/voices/en", b"voice"),
            ],
        );
        let installed = install_espeak_data(&archive, &root).expect("extract");
        assert_eq!(installed, root.join(ESPEAK_DATA_DIRECTORY));
        assert_eq!(
            fs::read(installed.join("en_dict")).expect("dictionary"),
            b"dictionary"
        );
        assert_eq!(
            fs::read(installed.join("voices").join("en")).expect("voice"),
            b"voice"
        );
        assert!(!root.join("jniLibs").exists());
        assert!(!root.join(STAGE_DIRECTORY).exists());
        assert!(espeak_data_installed(&root));
        assert_eq!(remove_espeak_data(&root), Ok(true));
        assert!(!espeak_data_installed(&root));
        assert!(!root.join(STAGE_DIRECTORY).exists());
        assert_eq!(remove_espeak_data(&root), Ok(false));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_traversal_absolute_paths_and_links_and_leaves_nothing_behind() {
        for entries in [
            vec![
                Entry::File("espeak-ng-data/en_dict", b"dictionary"),
                Entry::File("espeak-ng-data/../escaped", b"escape"),
            ],
            vec![
                Entry::File("espeak-ng-data/en_dict", b"dictionary"),
                Entry::File("/espeak-ng-data/absolute", b"absolute"),
            ],
            vec![
                Entry::File("espeak-ng-data/en_dict", b"dictionary"),
                Entry::Symlink("espeak-ng-data/link", "/etc/passwd"),
            ],
            vec![Entry::File("jniLibs/arm64-v8a/libttsespeak.so", b"library")],
        ] {
            let root = temp_root("espeak-reject");
            let archive = archive(&root, &entries);
            assert_eq!(
                install_espeak_data(&archive, &root),
                Err(EspeakBundleError::InvalidArchive)
            );
            assert!(!espeak_data_installed(&root));
            assert!(!root.join(STAGE_DIRECTORY).exists());
            assert!(!root.join("escaped").exists());
            assert!(!root.join(ESPEAK_DATA_DIRECTORY).exists());
            fs::remove_dir_all(root).expect("cleanup");
        }
    }

    #[test]
    fn a_leftover_stage_is_replaced_and_a_truncated_archive_fails_cleanly() {
        let root = temp_root("espeak-stage");
        fs::create_dir_all(root.join(STAGE_DIRECTORY).join("stale")).expect("stale stage");
        let archive = archive(
            &root,
            &[Entry::File("espeak-ng-data/en_dict", &[7_u8; 64 * 1024])],
        );
        let bytes = fs::read(&archive).expect("archive bytes");
        let truncated = root.join("truncated.tar.gz");
        fs::write(&truncated, &bytes[..bytes.len() / 2]).expect("truncated archive");
        assert!(install_espeak_data(&truncated, &root).is_err());
        assert!(!root.join(STAGE_DIRECTORY).exists());
        assert!(!espeak_data_installed(&root));
        install_espeak_data(&archive, &root).expect("extract after a failure");
        assert!(!root.join(ESPEAK_DATA_DIRECTORY).join("stale").exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn readiness_needs_the_data_only_on_android() {
        assert!(kokoro_model_ready(KokoroHostPlatform::Desktop, true, false));
        assert!(!kokoro_model_ready(
            KokoroHostPlatform::Android,
            true,
            false
        ));
        assert!(kokoro_model_ready(KokoroHostPlatform::Android, true, true));
        assert!(!kokoro_model_ready(
            KokoroHostPlatform::Android,
            false,
            true
        ));
        assert!(!kokoro_model_ready(
            KokoroHostPlatform::Desktop,
            false,
            false
        ));
    }

    #[test]
    fn the_bundle_url_is_built_from_the_pin() {
        assert_eq!(
            espeak_android_bundle_url(),
            "https://github.com/LettuceAI/app/releases/download/espeak-android-bundle-v2/kokoro-espeak-android-bundle.tar.gz"
        );
    }

    #[test]
    fn the_gradle_pin_matches_the_rust_pin() {
        let properties = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/tauri/gen/android/espeak-android-bundle.properties"),
        )
        .expect("Gradle bundle pin");
        let value = |key: &str| {
            properties
                .lines()
                .filter_map(|line| line.split_once('='))
                .find(|(name, _)| name.trim() == key)
                .map(|(_, value)| value.trim().to_owned())
        };
        assert_eq!(
            value("repository").as_deref(),
            Some(ESPEAK_ANDROID_BUNDLE_REPOSITORY)
        );
        assert_eq!(value("tag").as_deref(), Some(ESPEAK_ANDROID_BUNDLE_TAG));
        assert_eq!(value("asset").as_deref(), Some(ESPEAK_ANDROID_BUNDLE_ASSET));
        assert_eq!(
            value("sha256").as_deref(),
            Some(ESPEAK_ANDROID_BUNDLE_SHA256)
        );
    }
}
