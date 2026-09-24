//! The GGUF models folder: what is in it, deleting from it, moving it
//! (files and the model paths that point into it) and moving a model into it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use lettuce_models::ModelPathRelocation;
use lettuce_settings::{DeviceSettings, DeviceSettingsStore};
use lettuce_types::TimestampMillis;

use crate::llm_models_root;

/// A GGUF file found in the models folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedGguf {
    /// The repository the folder was named after.
    pub model_id: String,
    pub filename: String,
    pub path: String,
    pub size: u64,
    pub quantization: String,
    pub is_mmproj: bool,
    pub is_mtp: bool,
    /// A DFlash drafter by its GGUF metadata; always false on mobile.
    pub is_dflash: bool,
    pub architecture: Option<String>,
    pub context_length: Option<u64>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn is_dflash_drafter(path: &str) -> bool {
    lettuce_local_llm::dflash::model_is_dflash(path)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn is_dflash_drafter(_path: &str) -> bool {
    false
}

fn created(root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(root)
        .map_err(|error| format!("Failed to create GGUF models dir: {error}"))
}

/// The `.gguf` files one folder below `root`, with their header facts.
pub fn downloaded_ggufs(root: &Path) -> Result<Vec<DownloadedGguf>, String> {
    created(root)?;
    let entries =
        std::fs::read_dir(root).map_err(|error| format!("Failed to read models dir: {error}"))?;
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let folder = entry.path();
        if !folder.is_dir() {
            continue;
        }
        let model_id = entry.file_name().to_string_lossy().replace("--", "/");
        let Ok(files) = std::fs::read_dir(&folder) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            let filename = file.file_name().to_string_lossy().into_owned();
            if !filename.to_lowercase().ends_with(".gguf") {
                continue;
            }
            let meta = crate::models::model_runnability::local_gguf_meta(&path);
            let path_text = path.to_string_lossy().into_owned();
            let is_mmproj = filename.to_lowercase().contains("mmproj");
            found.push(DownloadedGguf {
                model_id: model_id.clone(),
                is_mmproj,
                is_dflash: !is_mmproj && is_dflash_drafter(&path_text),
                is_mtp: lettuce_model_hub::is_mtp_asset(&filename),
                size: file.metadata().map_or(0, |metadata| metadata.len()),
                quantization: lettuce_model_hub::extract_quantization(&path_text),
                architecture: meta.as_ref().and_then(|meta| meta.architecture.clone()),
                context_length: meta.as_ref().and_then(|meta| meta.context_length),
                filename,
                path: path_text,
            });
        }
    }
    Ok(found)
}

/// Deletes a model file inside `root` or one of `image_roots`, and its
/// folder when that is left empty.
pub fn delete_downloaded_model(
    root: &Path,
    image_roots: &[PathBuf],
    file_path: &str,
) -> Result<(), String> {
    let path = PathBuf::from(file_path);
    if !path.exists() {
        return Ok(());
    }
    let resolved = std::fs::canonicalize(&path)
        .map_err(|error| format!("Failed to delete model file: {error}"))?;
    let inside = |folder: &Path| {
        std::fs::canonicalize(folder).is_ok_and(|folder| resolved.starts_with(folder))
    };
    if !inside(root) && !image_roots.iter().any(|image| inside(image)) {
        return Err("Cannot delete files outside the models directory".to_owned());
    }
    std::fs::remove_file(&path).map_err(|error| format!("Failed to delete model file: {error}"))?;
    if let Some(parent) = path.parent()
        && parent != root
        && !image_roots.iter().any(|image| parent == image)
    {
        let _ = std::fs::remove_dir(parent);
    }
    Ok(())
}

/// The models folder in use, the app's own one, and how many entries the
/// folder in use holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmModelsDirInfo {
    pub path: PathBuf,
    pub default_path: PathBuf,
    pub is_custom: bool,
    pub model_count: u32,
}

fn default_root(app_folder: &Path) -> PathBuf {
    llm_models_root(&DeviceSettings::default(), app_folder)
}

fn count_models(dir: &Path) -> u32 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let count = entries
        .flatten()
        .filter(|entry| {
            let path = entry.path();
            path.is_dir()
                || path
                    .extension()
                    .is_some_and(|extension| !extension.is_empty())
        })
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

pub fn llm_models_dir_info(
    device: &DeviceSettings,
    app_folder: &Path,
) -> Result<LlmModelsDirInfo, String> {
    let path = llm_models_root(device, app_folder);
    created(&path)?;
    Ok(LlmModelsDirInfo {
        model_count: count_models(&path),
        is_custom: device
            .llm_models_dir
            .as_deref()
            .is_some_and(|dir| !dir.trim().is_empty()),
        default_path: default_root(app_folder),
        path,
    })
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(a), Ok(b)) if a == b
        )
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

fn copy_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    if source.is_dir() {
        std::fs::create_dir_all(destination).map_err(|error| error.to_string())?;
        for entry in std::fs::read_dir(source).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            copy_recursive(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::copy(source, destination).map_err(|error| error.to_string())?;
        if let Ok(file) = std::fs::File::open(destination) {
            file.sync_all().map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

/// Copies every entry of `from` into the empty-for-them `to`, lets
/// `rewire` point stored paths at the copies, then removes the originals;
/// a failure before that removes the copies again.
fn migrate_models_dir(
    from: &Path,
    to: &Path,
    rewire: impl FnOnce(&str, &str) -> Result<u32, String>,
) -> Result<(u32, u32), String> {
    if !from.exists() {
        return Ok((0, 0));
    }
    std::fs::create_dir_all(to)
        .map_err(|error| format!("Failed to create destination folder: {error}"))?;
    let mut sources: Vec<(OsString, PathBuf)> = Vec::new();
    for entry in
        std::fs::read_dir(from).map_err(|error| format!("Failed to read models folder: {error}"))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        sources.push((entry.file_name(), entry.path()));
    }
    if let Some((name, _)) = sources.iter().find(|(name, _)| to.join(name).exists()) {
        return Err(format!(
            "Destination already contains \"{}\". Pick an empty folder.",
            name.to_string_lossy()
        ));
    }
    let mut copies: Vec<PathBuf> = Vec::new();
    for (name, source) in &sources {
        let destination = to.join(name);
        if let Err(error) = copy_recursive(source, &destination) {
            for copy in &copies {
                let _ = remove_path(copy);
            }
            let _ = remove_path(&destination);
            return Err(error);
        }
        copies.push(destination);
    }
    if let Ok(folder) = std::fs::File::open(to) {
        let _ = folder.sync_all();
    }
    let rewired = match rewire(&from.to_string_lossy(), &to.to_string_lossy()) {
        Ok(count) => count,
        Err(error) => {
            for copy in &copies {
                let _ = remove_path(copy);
            }
            return Err(error);
        }
    };
    for (_, source) in &sources {
        let _ = remove_path(source);
    }
    Ok((u32::try_from(copies.len()).unwrap_or(u32::MAX), rewired))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmModelsDirChange {
    pub path: PathBuf,
    pub moved_entries: u32,
    pub rewired_models: u32,
}

/// Uses `new_dir` for GGUF models (the app's own folder again when it is
/// that one), first moving what the current folder holds and pointing the
/// stored model paths at the moved files when `move_existing` is set.
pub fn set_llm_models_dir<R>(
    repository: &R,
    app_folder: &Path,
    new_dir: &str,
    move_existing: bool,
    now: TimestampMillis,
) -> Result<LlmModelsDirChange, String>
where
    R: DeviceSettingsStore + ModelPathRelocation + ?Sized,
{
    let new_path = PathBuf::from(new_dir.trim());
    if new_path.as_os_str().is_empty() {
        return Err("New models folder path is empty".to_owned());
    }
    let old_path = llm_models_root(
        &repository
            .load_device_settings()
            .map_err(|error| error.to_string())?,
        app_folder,
    );
    std::fs::create_dir_all(&new_path)
        .map_err(|error| format!("Failed to create models folder: {error}"))?;
    let chosen = (!paths_equal(&new_path, &default_root(app_folder)))
        .then(|| new_path.to_string_lossy().into_owned());
    let persist = || -> Result<(), String> {
        let mut device = repository
            .load_device_settings()
            .map_err(|error| error.to_string())?;
        device.llm_models_dir.clone_from(&chosen);
        repository
            .save_device_settings(device)
            .map_err(|error| error.to_string())
    };
    if !move_existing || paths_equal(&old_path, &new_path) {
        persist()?;
        return Ok(LlmModelsDirChange {
            path: new_path,
            moved_entries: 0,
            rewired_models: 0,
        });
    }
    let (moved_entries, rewired_models) = migrate_models_dir(&old_path, &new_path, |old, new| {
        let relocate = |from: &str, to: &str| {
            repository
                .relocate_model_paths(
                    &|path| lettuce_models::rewrite_path_prefix(path, from, to),
                    now,
                )
                .map_err(|error| error.to_string())
        };
        let rewired = relocate(old, new)?;
        if let Err(error) = persist() {
            relocate(new, old)?;
            return Err(error);
        }
        Ok(rewired)
    })?;
    Ok(LlmModelsDirChange {
        path: new_path,
        moved_entries,
        rewired_models,
    })
}

/// Moves a model file into `root` (in a folder named after `model_name`,
/// else the file's stem) and returns where it now is; a file already inside
/// `root` stays where it is.
pub fn move_model_into_library(
    root: &Path,
    source_path: &str,
    model_name: Option<&str>,
) -> Result<String, String> {
    let source = PathBuf::from(source_path);
    if !source.exists() {
        return Err(format!("Source file does not exist: {source_path}"));
    }
    if !source.is_file() {
        return Err(format!("Source path is not a file: {source_path}"));
    }
    created(root)?;
    if source.starts_with(root) {
        return Ok(source_path.to_owned());
    }
    let filename = source
        .file_name()
        .ok_or_else(|| "Cannot determine filename from source path".to_owned())?;
    let folder = model_name
        .filter(|name| !name.trim().is_empty())
        .map_or_else(
            || {
                source
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            },
            |name| name.replace('/', "--"),
        );
    if !matches!(
        Path::new(&folder)
            .components()
            .collect::<Vec<_>>()
            .as_slice(),
        [std::path::Component::Normal(_)]
    ) {
        return Err("The model name cannot be used as a folder name".to_owned());
    }
    let destination_dir = root.join(folder);
    std::fs::create_dir_all(&destination_dir)
        .map_err(|error| format!("Failed to create destination directory: {error}"))?;
    let destination = destination_dir.join(filename);
    let moved = destination.to_string_lossy().into_owned();
    if destination.exists() {
        let _ = std::fs::remove_file(&source);
        return Ok(moved);
    }
    if std::fs::rename(&source, &destination).is_ok() {
        return Ok(moved);
    }
    std::fs::copy(&source, &destination)
        .map_err(|error| format!("Failed to copy model file: {error}"))?;
    std::fs::remove_file(&source)
        .map_err(|error| format!("File copied but failed to remove original: {error}"))?;
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use lettuce_models::ModelProfileRepository;

    use super::*;
    use crate::{GgufDownload, GgufModelSetup, register_downloaded_gguf};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lettuce-gguf-library-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn moving_the_folder_moves_the_files_and_the_model_paths() {
        let app = scratch("move");
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        let root = llm_models_root(&DeviceSettings::default(), &app);
        let download = GgufDownload {
            model_id: "org/m".to_owned(),
            model_file: "m-Q4_K_M.gguf".to_owned(),
            mmproj_file: Some("mmproj.gguf".to_owned()),
            mtp_file: None,
        };
        let installed = download.installed(&root);
        std::fs::create_dir_all(root.join("org--m")).expect("folder");
        std::fs::write(&installed.model_path, b"GGUF").expect("model");
        std::fs::write(installed.mmproj_path.as_deref().expect("mmproj"), b"GGUF").expect("mmproj");
        std::fs::write(root.join("notes.txt"), b"x").expect("notes");
        let model = register_downloaded_gguf(
            database,
            &root,
            &download,
            &GgufModelSetup::default(),
            TimestampMillis::new(2),
        )
        .expect("model")
        .profile;
        let listed = downloaded_ggufs(&root).expect("list");
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|file| file.model_id == "org/m"));
        let info = llm_models_dir_info(&DeviceSettings::default(), &app).expect("info");
        assert!(!info.is_custom);
        assert_eq!(info.model_count, 2);
        let target = app.join("elsewhere");
        let change = set_llm_models_dir(
            database,
            &app,
            &target.to_string_lossy(),
            true,
            TimestampMillis::new(3),
        )
        .expect("move");
        assert_eq!((change.moved_entries, change.rewired_models), (2, 1));
        assert!(!root.join("org--m").exists());
        let moved = ModelProfileRepository::get(database, model.id)
            .expect("get")
            .expect("model");
        let moved_download = download.installed(&target);
        assert_eq!(moved.external_model_id, moved_download.model_path);
        assert_eq!(
            moved.config.llama_cpp.mmproj_path,
            moved_download.mmproj_path
        );
        assert!(std::path::Path::new(&moved_download.model_path).exists());
        let device = database.load_device_settings().expect("device");
        assert_eq!(
            device.llm_models_dir.as_deref(),
            Some(target.to_string_lossy().as_ref())
        );
        std::fs::create_dir_all(&root).expect("recreate");
        set_llm_models_dir(
            database,
            &app,
            &root.to_string_lossy(),
            false,
            TimestampMillis::new(4),
        )
        .expect("back");
        assert_eq!(
            database
                .load_device_settings()
                .expect("device")
                .llm_models_dir,
            None
        );
        assert_eq!(
            delete_downloaded_model(&root, &[], &moved_download.model_path),
            Err("Cannot delete files outside the models directory".to_owned())
        );
        std::fs::remove_dir_all(&app).expect("cleanup");
    }

    fn gguf_with_u32(path: &Path, key: &str, value: u32) {
        let mut out = b"GGUF".to_vec();
        out.extend(3_u32.to_le_bytes());
        out.extend(0_u64.to_le_bytes());
        out.extend(1_u64.to_le_bytes());
        out.extend((key.len() as u64).to_le_bytes());
        out.extend(key.as_bytes());
        out.extend(4_u32.to_le_bytes());
        out.extend(value.to_le_bytes());
        std::fs::write(path, out).expect("gguf");
    }

    #[test]
    fn dflash_drafters_are_flagged_by_their_gguf_key() {
        let app = scratch("dflash");
        let folder = app.join("org--m");
        std::fs::create_dir_all(&folder).expect("folder");
        gguf_with_u32(&folder.join("m-Q4_K_M.gguf"), "llama.block_count", 32);
        gguf_with_u32(&folder.join("drafter.gguf"), "dflash.block_size", 16);
        gguf_with_u32(&folder.join("mmproj-m.gguf"), "dflash.block_size", 16);
        let listed = downloaded_ggufs(&app).expect("list");
        let flag = |name: &str| {
            listed
                .iter()
                .find(|file| file.filename == name)
                .map(|file| file.is_dflash)
                .expect("listed")
        };
        assert_eq!(listed.len(), 3);
        assert!(flag("drafter.gguf"));
        assert!(!flag("m-Q4_K_M.gguf"));
        assert!(!flag("mmproj-m.gguf"));
        std::fs::remove_dir_all(&app).expect("cleanup");
    }

    #[test]
    fn a_model_moved_into_the_library_gets_its_own_folder() {
        let app = scratch("adopt");
        let root = app.join("library");
        let source = app.join("Model.Q8_0.gguf");
        std::fs::write(&source, b"GGUF").expect("source");
        let moved = move_model_into_library(&root, &source.to_string_lossy(), Some("org/model"))
            .expect("moved");
        assert_eq!(
            PathBuf::from(&moved),
            root.join("org--model").join("Model.Q8_0.gguf")
        );
        assert!(!source.exists());
        assert_eq!(
            move_model_into_library(&root, &moved, None),
            Ok(moved.clone())
        );
        let outside = app.join("outside.gguf");
        std::fs::write(&outside, b"GGUF").expect("outside");
        let escaped = root.join("..").join("outside.gguf");
        assert_eq!(
            delete_downloaded_model(&root, &[], &escaped.to_string_lossy()),
            Err("Cannot delete files outside the models directory".to_owned())
        );
        assert_eq!(
            move_model_into_library(&root, &outside.to_string_lossy(), Some("..")),
            Err("The model name cannot be used as a folder name".to_owned())
        );
        assert!(outside.exists());
        delete_downloaded_model(&root, &[], &moved).expect("delete");
        assert!(!root.join("org--model").exists());
        std::fs::remove_dir_all(&app).expect("cleanup");
    }
}
