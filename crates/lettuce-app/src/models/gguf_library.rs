//! The GGUF models folder: what is in it, deleting from it, moving it
//! (files and the model paths that point into it) and moving a model into it.

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
    /// The path below the repository folder, `/`-separated.
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

fn hidden(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

/// The `.gguf` files below each repository folder of `root`, nested
/// folders included, with their header facts; hidden folders (partial
/// downloads) and the `skipped` folders (image models kept below the
/// models folder) are left out.
pub fn downloaded_ggufs(root: &Path, skipped: &[PathBuf]) -> Result<Vec<DownloadedGguf>, String> {
    created(root)?;
    let entries =
        std::fs::read_dir(root).map_err(|error| format!("Failed to read models dir: {error}"))?;
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let folder = entry.path();
        if !folder.is_dir()
            || hidden(&entry.file_name())
            || skipped.iter().any(|skipped| paths_equal(skipped, &folder))
        {
            continue;
        }
        let model_id = entry.file_name().to_string_lossy().replace("--", "/");
        let mut pending = vec![folder.clone()];
        while let Some(directory) = pending.pop() {
            let Ok(files) = std::fs::read_dir(&directory) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                if path.is_dir() {
                    if !hidden(&file.file_name()) {
                        pending.push(path);
                    }
                    continue;
                }
                let name = file.file_name().to_string_lossy().into_owned();
                if !name.to_lowercase().ends_with(".gguf") {
                    continue;
                }
                let filename = path
                    .strip_prefix(&folder)
                    .unwrap_or(&path)
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                let meta = crate::models::model_runnability::local_gguf_meta(&path);
                let path_text = path.to_string_lossy().into_owned();
                let is_mmproj = name.to_lowercase().contains("mmproj");
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
    }
    found.sort_by(|a, b| (&a.model_id, &a.filename).cmp(&(&b.model_id, &b.filename)));
    Ok(found)
}

/// Which path of a model points at a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPathField {
    Model,
    Mmproj,
    Mtp,
    Dflash,
}

/// A saved llama.cpp model (or, without an id, the global model defaults)
/// whose paths point at a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFileReference {
    pub model_profile_id: Option<lettuce_types::ModelProfileId>,
    pub display_name: Option<String>,
    pub fields: Vec<ModelPathField>,
}

fn same_file_path(candidate: Option<&str>, target: &str) -> bool {
    candidate.is_some_and(|candidate| {
        !candidate.is_empty() && candidate.replace('\\', "/") == target.replace('\\', "/")
    })
}

fn llama_fields(
    model_path: Option<&str>,
    llama: &lettuce_models::LlamaCppSettings,
    target: &str,
) -> Vec<ModelPathField> {
    [
        (ModelPathField::Model, model_path),
        (ModelPathField::Mmproj, llama.mmproj_path.as_deref()),
        (ModelPathField::Mtp, llama.mtp_model_path.as_deref()),
        (ModelPathField::Dflash, llama.dflash_model_path.as_deref()),
    ]
    .into_iter()
    .filter(|(_, path)| same_file_path(*path, target))
    .map(|(field, _)| field)
    .collect()
}

/// Every llama.cpp model and the global model defaults, read once to find
/// the ones whose paths point at a file.
#[derive(Debug)]
pub struct ModelFileReferences {
    profiles: Vec<(lettuce_models::ModelProfile, bool)>,
    defaults: lettuce_models::LlamaCppSettings,
}

impl ModelFileReferences {
    pub fn load<R>(repository: &R) -> Result<Self, String>
    where
        R: lettuce_models::ModelCatalog + lettuce_models::GlobalModelSettingsRepository + ?Sized,
    {
        let llama_accounts = repository
            .provider_accounts()
            .map_err(|error| error.to_string())?
            .into_iter()
            .filter(|account| account.protocol == lettuce_models::ProviderProtocol::LlamaCpp)
            .map(|account| account.id)
            .collect::<Vec<_>>();
        let profiles = repository
            .model_profiles()
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|profile| {
                let llama = llama_accounts.contains(&profile.provider_account_id);
                (profile, llama)
            })
            .collect();
        let (defaults, _) = repository
            .global_model_settings()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            profiles,
            defaults: defaults.llama_cpp,
        })
    }

    /// The models, and the global defaults, whose paths point at `path`.
    #[must_use]
    pub fn of(&self, path: &str) -> Vec<ModelFileReference> {
        let mut references = self
            .profiles
            .iter()
            .filter_map(|(profile, llama)| {
                let model_path = llama.then_some(profile.external_model_id.as_str());
                let fields = llama_fields(model_path, &profile.config.llama_cpp, path);
                (!fields.is_empty()).then(|| ModelFileReference {
                    model_profile_id: Some(profile.id),
                    display_name: Some(profile.display_name.clone()),
                    fields,
                })
            })
            .collect::<Vec<_>>();
        let fields = llama_fields(None, &self.defaults, path);
        if !fields.is_empty() {
            references.push(ModelFileReference {
                model_profile_id: None,
                display_name: None,
                fields,
            });
        }
        references
    }
}

/// The llama.cpp models, and the global model defaults, whose paths point
/// at `path`.
pub fn model_file_references<R>(
    repository: &R,
    path: &str,
) -> Result<Vec<ModelFileReference>, String>
where
    R: lettuce_models::ModelCatalog + lettuce_models::GlobalModelSettingsRepository + ?Sized,
{
    Ok(ModelFileReferences::load(repository)?.of(path))
}

pub const OUTSIDE_MODELS_FOLDER: &str = "Cannot delete files outside the models directory";

/// Whether `file_path` exists; an error when it is not inside `root` or one
/// of `image_roots`.
pub fn deletable_model(
    root: &Path,
    image_roots: &[PathBuf],
    file_path: &str,
) -> Result<bool, String> {
    let path = PathBuf::from(file_path);
    if !path.exists() {
        return Ok(false);
    }
    let resolved = std::fs::canonicalize(&path)
        .map_err(|error| format!("Failed to delete model file: {error}"))?;
    let inside = |folder: &Path| {
        std::fs::canonicalize(folder).is_ok_and(|folder| resolved.starts_with(folder))
    };
    if !inside(root) && !image_roots.iter().any(|image| inside(image)) {
        return Err(OUTSIDE_MODELS_FOLDER.to_owned());
    }
    Ok(true)
}

/// Deletes a model file inside `root` or one of `image_roots`, and its
/// folder when that is left empty.
pub fn delete_downloaded_model(
    root: &Path,
    image_roots: &[PathBuf],
    file_path: &str,
) -> Result<(), String> {
    if !deletable_model(root, image_roots, file_path)? {
        return Ok(());
    }
    let path = PathBuf::from(file_path);
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

pub(crate) fn paths_equal(a: &Path, b: &Path) -> bool {
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

/// The file in the destination folder that marks a move in progress: the
/// move it belongs to, the folder it copies from and each entry it copies
/// with the entry's size.
pub const MODELS_MOVE_MANIFEST: &str = ".lettuce-models-move.json";
const COPY_CHUNK_BYTES: usize = 1 << 20;

/// An entry's total bytes and file count, and for an original the newest
/// modification time of its files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Measure {
    bytes: u64,
    files: u64,
    modified: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ManifestEntry {
    name: String,
    measure: Measure,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct MoveManifest {
    move_id: String,
    from: String,
    entries: Vec<ManifestEntry>,
    #[serde(default)]
    rebound_files: Vec<ReboundManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ReboundManifest {
    relative_path: PathBuf,
    before_hash: String,
    after_hash: String,
}

fn measure(path: &Path) -> Option<Measure> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if metadata.is_dir() {
        let mut total = Measure {
            bytes: 0,
            files: 0,
            modified: None,
        };
        for entry in std::fs::read_dir(path).ok()? {
            let inner = measure(&entry.ok()?.path())?;
            total.bytes = total.bytes.saturating_add(inner.bytes);
            total.files = total.files.saturating_add(inner.files);
            total.modified = total.modified.max(inner.modified);
        }
        return Some(total);
    }
    Some(Measure {
        bytes: metadata.len(),
        files: 1,
        modified: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_secs()),
    })
}

/// What resolving a move left alone because it could not prove the entry
/// was its own copy (or that the copy is complete).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MoveResolution {
    pub committed: bool,
    pub kept: Vec<String>,
}

fn paths_only_rebinding(before: &[u8], after: &[u8], from: &Path, to: &Path) -> bool {
    fn rewrite(value: &mut serde_json::Value, from: &Path, to: &Path) {
        match value {
            serde_json::Value::Object(values) => {
                for (key, value) in values {
                    if key == "path" {
                        if let Some(path) = value.as_str()
                            && let Ok(relative) = Path::new(path).strip_prefix(from) {
                            *value = serde_json::Value::String(to.join(relative).to_string_lossy().into_owned());
                        }
                    } else { rewrite(value, from, to); }
                }
            }
            serde_json::Value::Array(values) => { for value in values { rewrite(value, from, to); } }
            _ => {}
        }
    }
    let Ok(mut before) = serde_json::from_slice::<serde_json::Value>(before) else { return false; };
    let Ok(after) = serde_json::from_slice::<serde_json::Value>(after) else { return false; };
    rewrite(&mut before, from, to);
    before == after
}

fn redundant_tree_with_rebindings(candidate: &Path, retained: &Path, partial: bool, bindings: Option<(&Path, &[ReboundManifest], bool)>) -> std::io::Result<bool> {
    use std::io::Read;
    let candidate_meta = std::fs::symlink_metadata(candidate)?;
    let retained_meta = std::fs::symlink_metadata(retained)?;
    if candidate_meta.is_dir() && retained_meta.is_dir() {
        for entry in std::fs::read_dir(candidate)? {
            let entry = entry?;
            if !redundant_tree_with_rebindings(&entry.path(), &retained.join(entry.file_name()), partial, bindings)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    if candidate_meta.is_file() && retained_meta.is_file()
        && let Some((root, records, reverse)) = bindings
        && let Ok(relative) = candidate.strip_prefix(root)
        && let Some(record) = records.iter().find(|record| record.relative_path == relative) {
        let candidate_bytes = std::fs::read(candidate)?;
        let retained_bytes = std::fs::read(retained)?;
        let candidate_hash = blake3::hash(&candidate_bytes).to_hex().to_string();
        let retained_hash = blake3::hash(&retained_bytes).to_hex().to_string();
        if reverse && candidate_hash == record.before_hash && retained_hash == record.before_hash { return Ok(true); }
        let other_root = retained.ancestors().nth(relative.components().count())
            .ok_or_else(|| std::io::Error::other("invalid rebound manifest path"))?;
        return Ok(if reverse {
            candidate_hash == record.after_hash && retained_hash == record.before_hash
                && paths_only_rebinding(&retained_bytes, &candidate_bytes, other_root, root)
        } else {
            candidate_hash == record.before_hash && retained_hash == record.after_hash
                && paths_only_rebinding(&candidate_bytes, &retained_bytes, root, other_root)
        });
    }
    if !candidate_meta.is_file() || !retained_meta.is_file()
        || candidate_meta.len() > retained_meta.len()
        || (!partial && candidate_meta.len() != retained_meta.len())
    {
        return Ok(false);
    }
    let mut candidate = std::fs::File::open(candidate)?;
    let mut retained = std::fs::File::open(retained)?;
    let mut candidate_buffer = vec![0_u8; COPY_CHUNK_BYTES];
    let mut retained_buffer = vec![0_u8; COPY_CHUNK_BYTES];
    loop {
        let read = candidate.read(&mut candidate_buffer)?;
        if read == 0 {
            return Ok(true);
        }
        retained.read_exact(&mut retained_buffer[..read])?;
        if candidate_buffer[..read] != retained_buffer[..read] {
            return Ok(false);
        }
    }
}

/// Removes the copies in `to` whose original is still in `from` unchanged.
fn undo_copies(from: &Path, to: &Path, entries: &[ManifestEntry], rebindings: &[ReboundManifest]) -> Vec<String> {
    let mut kept = Vec::new();
    for entry in entries {
        let copy = to.join(&entry.name);
        match std::fs::symlink_metadata(&copy) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                kept.push(entry.name.clone());
                continue;
            }
        }
        if measure(&from.join(&entry.name)) == Some(entry.measure)
            && matches!(redundant_tree_with_rebindings(&copy, &from.join(&entry.name), true, Some((to, rebindings, true))), Ok(true))
        {
            if let Err(error) = remove_path(&copy) {
                tracing::warn!(path = %copy.display(), %error, "a models folder copy could not be removed");
                kept.push(entry.name.clone());
            }
        } else {
            tracing::warn!(path = %copy.display(), "a models folder entry was kept: its original changed or is gone");
            kept.push(entry.name.clone());
        }
    }
    kept
}

/// Removes the originals in `from` whose copy in `to` is complete.
fn remove_originals(from: &Path, to: &Path, entries: &[ManifestEntry], rebindings: &[ReboundManifest]) -> Vec<String> {
    let mut kept = Vec::new();
    for entry in entries {
        let original = from.join(&entry.name);
        match std::fs::symlink_metadata(&original) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                kept.push(entry.name.clone());
                continue;
            }
        }
        let copied = measure(&to.join(&entry.name)).is_some_and(|copy| {
            (copy.bytes == entry.measure.bytes || rebindings.iter().any(|record| record.relative_path.starts_with(&entry.name)))
                && copy.files == entry.measure.files
        });
        if copied && matches!(redundant_tree_with_rebindings(&original, &to.join(&entry.name), false, Some((from, rebindings, false))), Ok(true)) {
            if let Err(error) = remove_path(&original) {
                tracing::warn!(path = %original.display(), %error, "a moved original could not be removed");
                kept.push(entry.name.clone());
            }
        } else {
            tracing::warn!(path = %original.display(), "an original was kept: its copy is missing or incomplete");
            kept.push(entry.name.clone());
        }
    }
    kept
}

fn read_manifest(to: &Path) -> Result<Option<MoveManifest>, String> {
    match std::fs::read(to.join(MODELS_MOVE_MANIFEST)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("The move manifest is unreadable: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn record_manifest_rebinding(to: &Path, move_id: &str, path: &Path, before: &[u8], after: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let relative = path.strip_prefix(to).map_err(|error| error.to_string())?;
    if !relative.components().all(|part| matches!(part, std::path::Component::Normal(_))) {
        return Err("the rebound manifest is outside the target folder".into());
    }
    if std::fs::read(path).map_err(|error| error.to_string())? != before {
        return Err("the copied model manifest changed before relocation".into());
    }
    let mut manifest = read_manifest(to)?.filter(|manifest| manifest.move_id == move_id)
        .ok_or_else(|| "the move manifest is missing".to_owned())?;
    manifest.rebound_files.push(ReboundManifest {
        relative_path: relative.to_owned(), before_hash: blake3::hash(before).to_hex().to_string(),
        after_hash: blake3::hash(after).to_hex().to_string(),
    });
    let bytes = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
    let next = to.join(format!("{MODELS_MOVE_MANIFEST}.next"));
    let mut file = std::fs::File::create(&next).map_err(|error| error.to_string())?;
    file.write_all(&bytes).and_then(|()| file.sync_all()).map_err(|error| error.to_string())?;
    std::fs::rename(next, to.join(MODELS_MOVE_MANIFEST)).map_err(|error| error.to_string())?;
    #[cfg(not(target_os = "windows"))]
    std::fs::File::open(to).and_then(|file| file.sync_all()).map_err(|error| error.to_string())?;
    Ok(())
}

/// Removes the manifest `move_id` left in `to` without applying it, for a
/// move already resolved.
pub fn discard_models_folder_manifest(to: &Path, move_id: &str) -> Result<bool, String> {
    match read_manifest(to)? {
        Some(manifest) if manifest.move_id == move_id => {
            std::fs::remove_file(to.join(MODELS_MOVE_MANIFEST))
                .map_err(|error| error.to_string())?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Why a models folder move stopped; a move that stops removes its copies.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FolderMoveError {
    #[error("New models folder path is empty")]
    EmptyPath,
    #[error("The new models folder is inside the current one")]
    DestinationInsideSource,
    #[error("Destination already contains \"{0}\". Pick an empty folder.")]
    DestinationNotEmpty(String),
    #[error("the models folder move was cancelled")]
    Cancelled,
    #[error("Failed to copy the models: {0}")]
    Copy(String),
    #[error("The model paths could not be saved: {0}")]
    Storage(String),
}

fn copy_error(error: std::io::Error) -> FolderMoveError {
    FolderMoveError::Copy(error.to_string())
}

/// Copies `source` to `destination` in chunks, checking `cancelled` before
/// each chunk and each directory entry.
fn copy_cancellable(
    source: &Path,
    destination: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), FolderMoveError> {
    use std::io::{Read, Write};
    if cancelled() {
        return Err(FolderMoveError::Cancelled);
    }
    if source.is_dir() {
        std::fs::create_dir_all(destination).map_err(copy_error)?;
        for entry in std::fs::read_dir(source).map_err(copy_error)? {
            let entry = entry.map_err(copy_error)?;
            copy_cancellable(
                &entry.path(),
                &destination.join(entry.file_name()),
                cancelled,
            )?;
        }
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(copy_error)?;
    }
    let mut input = std::fs::File::open(source).map_err(copy_error)?;
    let mut output = std::fs::File::create_new(destination).map_err(copy_error)?;
    let mut buffer = vec![0_u8; COPY_CHUNK_BYTES];
    loop {
        if cancelled() {
            return Err(FolderMoveError::Cancelled);
        }
        let read = input.read(&mut buffer).map_err(copy_error)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read]).map_err(copy_error)?;
    }
    output.sync_all().map_err(copy_error)
}

/// Refuses a destination inside the source folder (the copy would copy
/// itself) and one that already holds an entry the source has.
pub fn check_folder_move(from: &Path, to: &Path) -> Result<(), FolderMoveError> {
    let resolve = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let (from_resolved, to_resolved) = (resolve(from), resolve(to));
    if to_resolved != from_resolved && to_resolved.starts_with(&from_resolved) {
        return Err(FolderMoveError::DestinationInsideSource);
    }
    let Ok(entries) = std::fs::read_dir(from) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if to.join(entry.file_name()).exists() {
            return Err(FolderMoveError::DestinationNotEmpty(
                entry.file_name().to_string_lossy().into_owned(),
            ));
        }
    }
    Ok(())
}

/// Finishes or undoes the move `move_id` a crash or shutdown interrupted,
/// from its manifest in `to`: when the stored folder is `to` the move
/// committed and each original whose copy is complete goes, else each copy
/// whose original is still there unchanged goes. Entries it cannot prove
/// are kept and reported. `None` when `to` holds no manifest of this move.
pub fn recover_models_folder_move<R>(
    repository: &R,
    app_folder: &Path,
    to: &Path,
    move_id: &str,
) -> Result<Option<MoveResolution>, String>
where
    R: DeviceSettingsStore + ?Sized,
{
    let Some(manifest) = read_manifest(to)?.filter(|manifest| manifest.move_id == move_id) else {
        return Ok(None);
    };
    let current = llm_models_root(
        &repository
            .load_device_settings()
            .map_err(|error| error.to_string())?,
        app_folder,
    );
    let from = Path::new(&manifest.from);
    let committed = paths_equal(&current, to);
    let kept = if committed {
        remove_originals(from, to, &manifest.entries, &manifest.rebound_files)
    } else {
        undo_copies(from, to, &manifest.entries, &manifest.rebound_files)
    };
    if let Err(error) = std::fs::remove_file(to.join(MODELS_MOVE_MANIFEST)) {
        tracing::warn!(%error, "a resolved move manifest could not be removed");
    }
    Ok(Some(MoveResolution { committed, kept }))
}

/// Copies every entry of `from` into `to` under a manifest, lets `commit`
/// point the stored paths and the folder setting at the copies, then removes
/// the originals whose copies are complete and the manifest; a manifest that
/// cannot be removed then is left for the next start. A failure or
/// cancellation before the commit removes the copies whose originals are
/// unchanged.
fn migrate_models_dir(
    move_id: &str,
    from: &Path,
    to: &Path,
    cancelled: &dyn Fn() -> bool,
    commit: impl FnOnce() -> Result<u32, FolderMoveError>,
) -> Result<(u32, u32), FolderMoveError> {
    std::fs::create_dir_all(to).map_err(copy_error)?;
    check_folder_move(from, to)?;
    let mut entries: Vec<ManifestEntry> = Vec::new();
    if from.exists() {
        for entry in std::fs::read_dir(from).map_err(copy_error)? {
            let name = entry.map_err(copy_error)?.file_name();
            let name = name.to_string_lossy().into_owned();
            if name == MODELS_MOVE_MANIFEST {
                continue;
            }
            let measure = measure(&from.join(&name))
                .ok_or_else(|| FolderMoveError::Copy(format!("\"{name}\" could not be read")))?;
            entries.push(ManifestEntry { name, measure });
        }
    }
    let manifest_path = to.join(MODELS_MOVE_MANIFEST);
    let manifest = serde_json::to_vec(&MoveManifest {
        move_id: move_id.to_owned(),
        from: from.to_string_lossy().into_owned(),
        entries: entries.clone(),
        rebound_files: Vec::new(),
    })
    .map_err(|error| FolderMoveError::Copy(error.to_string()))?;
    {
        use std::io::Write;
        let mut file = std::fs::File::create_new(&manifest_path).map_err(copy_error)?;
        file.write_all(&manifest).map_err(copy_error)?;
        file.sync_all().map_err(copy_error)?;
    }
    let undo = |error: FolderMoveError| {
        let rebindings = match read_manifest(to) {
            Ok(Some(manifest)) => manifest.rebound_files,
            Ok(None) => return FolderMoveError::Copy(format!("{error}; the move journal is missing, so copies were preserved")),
            Err(cause) => return FolderMoveError::Copy(format!("{error}; copies were preserved because the move journal could not be read: {cause}")),
        };
        let kept = undo_copies(from, to, &entries, &rebindings);
        if kept.is_empty() {
            let _ = std::fs::remove_file(&manifest_path);
        }
        error
    };
    for entry in &entries {
        copy_cancellable(&from.join(&entry.name), &to.join(&entry.name), cancelled)
            .map_err(undo)?;
    }
    if let Ok(folder) = std::fs::File::open(to) {
        let _ = folder.sync_all();
    }
    if cancelled() {
        return Err(undo(FolderMoveError::Cancelled));
    }
    let rewired = commit().map_err(undo)?;
    let recorded = read_manifest(to).map_err(FolderMoveError::Copy)?
        .ok_or_else(|| FolderMoveError::Copy("the committed move manifest is missing".into()))?;
    let kept = remove_originals(from, to, &entries, &recorded.rebound_files);
    if kept.is_empty()
        && let Err(error) = std::fs::remove_file(&manifest_path)
    {
        tracing::warn!(%error, "the move manifest is left for the next start");
    }
    Ok((u32::try_from(entries.len()).unwrap_or(u32::MAX), rewired))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmModelsDirChange {
    pub path: PathBuf,
    pub moved_entries: u32,
    pub rewired_models: u32,
}

/// Uses `new_dir` for GGUF models (the app's own folder again when it is
/// that one), first moving what the current folder holds and pointing the
/// stored model paths at the moved files when `move_existing` is set. The
/// paths and the folder setting change in one transaction; `cancelled` is
/// checked between files and chunks.
pub fn set_llm_models_dir<R>(
    repository: &R,
    app_folder: &Path,
    new_dir: &str,
    move_existing: bool,
    now: TimestampMillis,
    move_id: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<LlmModelsDirChange, FolderMoveError>
where
    R: DeviceSettingsStore + ModelPathRelocation + ?Sized,
{
    let new_path = PathBuf::from(new_dir.trim());
    if new_path.as_os_str().is_empty() {
        return Err(FolderMoveError::EmptyPath);
    }
    let storage = |error: &dyn std::fmt::Display| FolderMoveError::Storage(error.to_string());
    let device = repository
        .load_device_settings()
        .map_err(|error| storage(&error))?;
    let old_path = llm_models_root(&device, app_folder);
    std::fs::create_dir_all(&new_path).map_err(copy_error)?;
    let chosen = (!paths_equal(&new_path, &default_root(app_folder)))
        .then(|| new_path.to_string_lossy().into_owned());
    let with_folder = |mut device: DeviceSettings| {
        device.llm_models_dir.clone_from(&chosen);
        device
    };
    if !move_existing || paths_equal(&old_path, &new_path) {
        repository
            .save_device_settings(with_folder(device))
            .map_err(|error| storage(&error))?;
        return Ok(LlmModelsDirChange {
            path: new_path,
            moved_entries: 0,
            rewired_models: 0,
        });
    }
    let (old, new) = (
        old_path.to_string_lossy().into_owned(),
        new_path.to_string_lossy().into_owned(),
    );
    let (moved_entries, rewired_models) =
        migrate_models_dir(move_id, &old_path, &new_path, cancelled, || {
            let device = repository
                .load_device_settings()
                .map_err(|error| storage(&error))?;
            let roots = crate::speech::speech_roots::retained_model_roots(&device, app_folder);
            crate::speech::speech_roots::rebind_memory_model_manifests(&roots, &old_path, &new_path,
                |path, before, after| record_manifest_rebinding(&new_path, move_id, path, before, after))
                .map_err(FolderMoveError::Storage)?;
            repository
                .relocate_model_paths_and_save_device(
                    &|path| lettuce_models::rewrite_path_prefix(path, &old, &new),
                    {
                        let mut roots = crate::speech::speech_roots::retained_model_roots(&device, app_folder);
                        for root in [&mut roots.whisper, &mut roots.kokoro, &mut roots.embedding, &mut roots.thymos] {
                            if let Some(path) = root.as_deref().and_then(|path| lettuce_models::rewrite_path_prefix(path, &old, &new)) {
                                *root = Some(path);
                            }
                        }
                        let mut moved = with_folder(device);
                        moved.retained_model_roots = roots;
                        moved
                    },
                    now,
                )
                .map_err(|error| storage(&error))
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
    let _guard = LIBRARY_MOVES
        .lock()
        .map_err(|_| "Another model move failed while holding the library".to_owned())?;
    if std::fs::symlink_metadata(&destination).is_ok() {
        return remove_duplicate_original(&source, &destination, &moved);
    }
    match publish_no_clobber(&source, &destination) {
        Ok(()) => return Ok(moved),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return remove_duplicate_original(&source, &destination, &moved);
        }
        Err(_) => {}
    }
    let staged = destination_dir.join(format!(
        ".{}.{}.partial",
        filename.to_string_lossy(),
        uuid::Uuid::new_v4().simple()
    ));
    let published = copy_verified(&source, &staged).and_then(|()| {
        publish_no_clobber(&staged, &destination).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                format!("A different file appeared at {moved}; the original was kept")
            } else {
                format!("Failed to move the copied model into place: {error}")
            }
        })
    });
    if let Err(error) = published {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }
    std::fs::remove_file(&source)
        .map_err(|error| format!("File copied but failed to remove original: {error}"))?;
    Ok(moved)
}

/// Serializes library moves in this process, so the existence check and the
/// rename that follows it cannot interleave with another move.
static LIBRARY_MOVES: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn remove_duplicate_original(
    source: &Path,
    destination: &Path,
    moved: &str,
) -> Result<String, String> {
    if !same_contents(source, destination)? {
        return Err(format!(
            "A different file already exists at {moved}; the original was kept"
        ));
    }
    std::fs::remove_file(source)
        .map_err(|error| format!("Failed to remove the duplicate original: {error}"))?;
    Ok(moved.to_owned())
}

/// Moves `from` to `to` without ever replacing an existing `to`: a hard link
/// where the filesystem allows one, else an existence check and a rename under
/// [`LIBRARY_MOVES`], which the caller holds.
fn publish_no_clobber(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::hard_link(from, to) {
        Ok(()) => {
            let _ = std::fs::remove_file(from);
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Err(error),
        Err(_) => {}
    }
    if std::fs::symlink_metadata(to).is_ok() {
        return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
    }
    std::fs::rename(from, to)
}

/// Copies `source` into a new file at `staged`, flushes it to disk and checks
/// that the copy holds exactly the source's bytes.
fn copy_verified(source: &Path, staged: &Path) -> Result<(), String> {
    let mut input = std::fs::File::open(source)
        .map_err(|error| format!("Failed to read the model file: {error}"))?;
    let mut output = std::fs::File::create_new(staged)
        .map_err(|error| format!("Failed to copy model file: {error}"))?;
    std::io::copy(&mut input, &mut output)
        .map_err(|error| format!("Failed to copy model file: {error}"))?;
    output
        .sync_all()
        .map_err(|error| format!("Failed to flush the copied model file: {error}"))?;
    if !same_contents(source, staged)? {
        return Err("The copied model file does not match the original".to_owned());
    }
    Ok(())
}

fn same_contents(left: &Path, right: &Path) -> Result<bool, String> {
    use std::io::Read;
    let read_error = |error: std::io::Error| format!("Failed to compare model files: {error}");
    let mut left = std::fs::File::open(left).map_err(read_error)?;
    let mut right = std::fs::File::open(right).map_err(read_error)?;
    if left.metadata().map_err(read_error)?.len() != right.metadata().map_err(read_error)?.len() {
        return Ok(false);
    }
    let mut left_buffer = vec![0_u8; 1 << 20];
    let mut right_buffer = vec![0_u8; 1 << 20];
    loop {
        let read = left.read(&mut left_buffer).map_err(read_error)?;
        if read == 0 {
            return Ok(right.read(&mut right_buffer[..1]).map_err(read_error)? == 0);
        }
        right
            .read_exact(&mut right_buffer[..read])
            .map_err(read_error)?;
        if left_buffer[..read] != right_buffer[..read] {
            return Ok(false);
        }
    }
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
    fn custom_root_overlap_rebinds_speech_and_embedding_manifests_and_device_roots() {
        use lettuce_model_hub::{EmbeddingInstallStore, EmbeddingModelFamily, InstalledEmbeddingManifest,
            InstalledModelArtifact, InstalledWhisperManifest, WhisperModelRepository};
        let scratch = scratch("retained-overlap");
        let app = scratch.join("app");
        let target = scratch.join("destination");
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        let whisper = crate::whisper_models_root(&app);
        let whisper_file = whisper.join("tiny.en").join("ggml-tiny.en.bin");
        std::fs::create_dir_all(whisper_file.parent().expect("parent")).expect("folder");
        std::fs::write(&whisper_file, b"retained whisper").expect("whisper");
        let admitted = InstalledWhisperManifest::inspect_legacy(&whisper, &whisper_file, TimestampMillis::new(2)).expect("manifest");
        database.admit_whisper_model(admitted.clone()).expect("admit");
        let embedding_root = crate::embedding_models_root(&app);
        let files = embedding_root.join(EmbeddingModelFamily::LettuceEmbV4.install_dir());
        std::fs::create_dir_all(&files).expect("embedding folder");
        std::fs::write(files.join("model.onnx"), b"model").expect("model");
        std::fs::write(files.join("tokenizer.json"), b"tokenizer").expect("tokenizer");
        let manifest = InstalledEmbeddingManifest {
            family: EmbeddingModelFamily::LettuceEmbV4, source_revision: "legacy-import".into(),
            model: InstalledModelArtifact::inspect(files.join("model.onnx")).expect("model"),
            tokenizer: InstalledModelArtifact::inspect(files.join("tokenizer.json")).expect("tokenizer"),
            calibration: None, max_sequence_length: 512, native_dimensions: 768,
        };
        EmbeddingInstallStore::new(&embedding_root).record(&manifest).expect("record");
        let mut device = database.load_device_settings().expect("device");
        device.llm_models_dir = Some(app.to_string_lossy().into_owned());
        database.save_device_settings(device).expect("custom root");
        set_llm_models_dir(database, &app, target.to_str().expect("target"), true,
            TimestampMillis::new(3), "retained-overlap", &|| false).expect("move");
        let device = database.load_device_settings().expect("new device");
        let roots = crate::speech::speech_roots::retained_model_roots(&device, &app);
        assert_eq!(roots.whisper.as_deref(), target.join("models/whisper").to_str());
        assert_eq!(roots.embedding.as_deref(), target.join("models/embedding").to_str());
        assert_eq!(roots.kokoro.as_deref(), target.join("kokoro").to_str());
        assert_eq!(roots.thymos.as_deref(), target.join("models/thymos").to_str());
        let rebound = database.get_whisper_model(&admitted.model_id).expect("whisper").expect("model");
        rebound.verify_contents().expect("verified moved whisper");
        assert_eq!(rebound.model.blake3, admitted.model.blake3);
        let moved = EmbeddingInstallStore::new(target.join("models/embedding"))
            .manifest(EmbeddingModelFamily::LettuceEmbV4).expect("embedding manifest").expect("model");
        moved.verify().expect("verified embedding");
        assert_eq!(moved.model.blake3, manifest.model.blake3);
        assert!(!app.join("models").exists(), "verified originals should be removed after rebinding");
        std::fs::remove_dir_all(scratch).expect("cleanup");
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
        .expect("model");
        let listed = downloaded_ggufs(&root, &[]).expect("list");
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
            "move-1",
            &|| false,
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
            "move-2",
            &|| false,
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
        let listed = downloaded_ggufs(&app, &[]).expect("list");
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

    fn library_with_model(label: &str) -> (PathBuf, PathBuf, crate::AppBackend) {
        let app = scratch(label);
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let root = llm_models_root(&DeviceSettings::default(), &app);
        std::fs::create_dir_all(root.join("org--m")).expect("folder");
        std::fs::write(root.join("org--m").join("m.gguf"), vec![7_u8; 3 << 20]).expect("model");
        std::fs::write(root.join("notes.txt"), b"x").expect("notes");
        (app, root, backend)
    }

    #[test]
    fn a_cancelled_move_removes_its_copies_and_keeps_everything_else() {
        let (app, root, backend) = library_with_model("cancel");
        let database = backend.database();
        let target = app.join("elsewhere");
        let checks = std::sync::atomic::AtomicU32::new(0);
        let cancel_on_third = || checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 2;
        assert_eq!(
            set_llm_models_dir(
                database,
                &app,
                &target.to_string_lossy(),
                true,
                TimestampMillis::new(3),
                "move-1",
                &cancel_on_third,
            ),
            Err(FolderMoveError::Cancelled)
        );
        assert!(root.join("org--m").join("m.gguf").exists());
        assert!(!target.join("org--m").exists());
        assert!(!target.join("notes.txt").exists());
        assert!(!target.join(MODELS_MOVE_MANIFEST).exists());
        assert_eq!(
            database
                .load_device_settings()
                .expect("device")
                .llm_models_dir,
            None
        );
        let inside = root.join("nested");
        assert_eq!(
            set_llm_models_dir(
                database,
                &app,
                &inside.to_string_lossy(),
                true,
                TimestampMillis::new(4),
                "move-2",
                &|| false,
            ),
            Err(FolderMoveError::DestinationInsideSource),
            "legacy copied the folder into itself"
        );
        std::fs::create_dir_all(target.join("notes.txt")).expect("clash");
        assert_eq!(
            check_folder_move(&root, &target),
            Err(FolderMoveError::DestinationNotEmpty("notes.txt".to_owned()))
        );
        std::fs::remove_dir_all(&app).expect("cleanup");
    }

    fn manifest_of(root: &Path, names: &[&str]) -> Vec<u8> {
        serde_json::to_vec(&MoveManifest {
            move_id: "move-1".to_owned(),
            from: root.to_string_lossy().into_owned(),
            rebound_files: Vec::new(),
            entries: names
                .iter()
                .map(|name| ManifestEntry {
                    name: (*name).to_owned(),
                    measure: measure(&root.join(name)).expect("original"),
                })
                .collect(),
        })
        .expect("manifest")
    }

    #[test]
    fn an_interrupted_move_is_undone_or_finished_at_the_next_start() {
        let (app, root, backend) = library_with_model("crash");
        let database = backend.database();
        let target = app.join("elsewhere");
        let manifest = manifest_of(&root, &["org--m", "notes.txt"]);
        std::fs::create_dir_all(target.join("org--m")).expect("partial copy");
        std::fs::write(target.join("org--m").join("m.gguf"), [7_u8; 4]).expect("partial");
        std::fs::write(target.join("unrelated.gguf"), b"mine").expect("unrelated");
        std::fs::write(target.join(MODELS_MOVE_MANIFEST), &manifest).expect("manifest");
        assert_eq!(
            recover_models_folder_move(database, &app, &target, "other-move"),
            Ok(None),
            "a manifest of another move is not applied"
        );
        assert_eq!(
            recover_models_folder_move(database, &app, &target, "move-1"),
            Ok(Some(MoveResolution {
                committed: false,
                kept: Vec::new(),
            }))
        );
        assert!(!target.join("org--m").exists(), "the partial copy is gone");
        assert!(target.join("unrelated.gguf").exists());
        assert!(root.join("org--m").join("m.gguf").exists());
        assert!(!target.join(MODELS_MOVE_MANIFEST).exists());
        assert_eq!(
            recover_models_folder_move(database, &app, &target, "move-1"),
            Ok(None)
        );
        copy_cancellable(&root.join("org--m"), &target.join("org--m"), &|| false).expect("copy");
        std::fs::write(target.join(MODELS_MOVE_MANIFEST), &manifest).expect("manifest");
        let mut device = database.load_device_settings().expect("device");
        device.llm_models_dir = Some(target.to_string_lossy().into_owned());
        database.save_device_settings(device).expect("committed");
        assert_eq!(
            recover_models_folder_move(database, &app, &target, "move-1"),
            Ok(Some(MoveResolution {
                committed: true,
                kept: vec!["notes.txt".to_owned()],
            })),
            "an original whose copy is missing stays"
        );
        assert!(target.join("org--m").join("m.gguf").exists());
        assert!(
            !root.join("org--m").exists(),
            "the committed move's originals go"
        );
        assert!(root.join("notes.txt").exists());
        std::fs::remove_dir_all(&app).expect("cleanup");
    }

    #[test]
    fn recovery_preserves_changed_contents_on_both_sides() {
        for committed in [false, true] {
            let (app, root, backend) = library_with_model("changed-recovery");
            let target = app.join("elsewhere");
            let manifest = manifest_of(&root, &["notes.txt"]);
            std::fs::create_dir_all(&target).expect("target");
            std::fs::write(target.join("notes.txt"), b"x").expect("copy");
            std::fs::write(target.join(MODELS_MOVE_MANIFEST), manifest).expect("manifest");
            if committed {
                let mut device = backend.database().load_device_settings().expect("device");
                device.llm_models_dir = Some(target.to_string_lossy().into_owned());
                backend.database().save_device_settings(device).expect("commit");
                std::fs::write(root.join("notes.txt"), b"y").expect("changed source");
            } else {
                std::fs::write(target.join("notes.txt"), b"y").expect("changed destination");
            }
            let resolution = recover_models_folder_move(backend.database(), &app, &target, "move-1")
                .expect("recover").expect("resolution");
            assert_eq!(resolution.kept, ["notes.txt"]);
            assert!(root.join("notes.txt").exists());
            assert!(target.join("notes.txt").exists());
            std::fs::remove_dir_all(&app).expect("cleanup");
        }
    }

    #[test]
    fn undoing_a_move_keeps_an_entry_whose_original_is_gone() {
        let (app, root, backend) = library_with_model("hand-moved");
        let database = backend.database();
        let target = app.join("elsewhere");
        let manifest = manifest_of(&root, &["org--m", "notes.txt"]);
        std::fs::create_dir_all(&target).expect("target");
        std::fs::write(target.join(MODELS_MOVE_MANIFEST), &manifest).expect("manifest");
        std::fs::rename(root.join("org--m"), target.join("org--m")).expect("moved by hand");
        std::fs::write(target.join("notes.txt"), b"x").expect("copy");
        std::fs::write(root.join("notes.txt"), b"changed").expect("original changed");
        assert_eq!(
            recover_models_folder_move(database, &app, &target, "move-1"),
            Ok(Some(MoveResolution {
                committed: false,
                kept: vec!["org--m".to_owned(), "notes.txt".to_owned()],
            }))
        );
        assert_eq!(
            std::fs::read(target.join("org--m").join("m.gguf"))
                .expect("the only copy stays")
                .len(),
            3 << 20
        );
        assert!(target.join("notes.txt").exists());
        std::fs::remove_dir_all(&app).expect("cleanup");
    }

    #[test]
    fn nested_repository_files_are_listed_and_the_image_folder_is_not() {
        let app = scratch("nested");
        let root = app.join("library");
        let nested = root.join("org--m").join("Q4");
        std::fs::create_dir_all(&nested).expect("nested");
        std::fs::write(nested.join("m-Q4_K_M.gguf"), b"GGUF").expect("nested model");
        std::fs::create_dir_all(root.join("org--m").join("mtp")).expect("mtp");
        std::fs::write(root.join("org--m").join("mtp").join("draft.gguf"), b"GGUF").expect("mtp");
        std::fs::write(root.join("org--m").join("top.gguf"), b"GGUF").expect("top");
        let partial = root.join(".downloads");
        std::fs::create_dir_all(&partial).expect("partials");
        std::fs::write(partial.join("x.gguf"), b"GGUF").expect("partial");
        let image = root.join("image").join("components").join("abc");
        std::fs::create_dir_all(&image).expect("image");
        std::fs::write(image.join("encoder.gguf"), b"GGUF").expect("encoder");
        let listed = downloaded_ggufs(&root, &[root.join("image")]).expect("list");
        assert_eq!(
            listed
                .iter()
                .map(|file| (file.model_id.as_str(), file.filename.as_str(), file.is_mtp))
                .collect::<Vec<_>>(),
            [
                ("org/m", "Q4/m-Q4_K_M.gguf", false),
                ("org/m", "mtp/draft.gguf", true),
                ("org/m", "top.gguf", false),
            ],
            "legacy listed only files directly inside a repository folder"
        );
        assert_eq!(listed[0].quantization, "Q4_K_M");
        std::fs::remove_dir_all(&app).expect("cleanup");
    }

    #[test]
    fn references_name_every_llama_model_path_and_the_global_defaults() {
        use lettuce_models::GlobalModelSettingsRepository;
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        let root = Path::new("/models");
        let download = GgufDownload {
            model_id: "org/m".to_owned(),
            model_file: "m.gguf".to_owned(),
            mmproj_file: Some("mmproj.gguf".to_owned()),
            mtp_file: None,
        };
        let installed = download.installed(root);
        let model = register_downloaded_gguf(
            database,
            root,
            &download,
            &GgufModelSetup::default(),
            TimestampMillis::new(2),
        )
        .expect("model");
        let mmproj = installed.mmproj_path.clone().expect("mmproj");
        let (mut defaults, revision) = database.global_model_settings().expect("defaults");
        defaults.llama_cpp.mmproj_path = Some(mmproj.clone());
        database
            .save_global_model_settings(defaults, revision, TimestampMillis::new(3))
            .expect("save defaults");
        assert_eq!(
            model_file_references(database, &installed.model_path).expect("model"),
            [ModelFileReference {
                model_profile_id: Some(model.id),
                display_name: Some(model.display_name.clone()),
                fields: vec![ModelPathField::Model],
            }]
        );
        assert_eq!(
            model_file_references(database, &mmproj).expect("mmproj"),
            [
                ModelFileReference {
                    model_profile_id: Some(model.id),
                    display_name: Some(model.display_name.clone()),
                    fields: vec![ModelPathField::Mmproj],
                },
                ModelFileReference {
                    model_profile_id: None,
                    display_name: None,
                    fields: vec![ModelPathField::Mmproj],
                },
            ]
        );
        assert!(
            model_file_references(database, "/elsewhere.gguf")
                .expect("none")
                .is_empty()
        );
    }

    #[test]
    fn a_different_file_at_the_destination_keeps_the_original() {
        let app = scratch("clash");
        let root = app.join("library");
        let source = app.join("Model.gguf");
        std::fs::write(&source, b"GGUF full model").expect("source");
        std::fs::create_dir_all(root.join("org--model")).expect("folder");
        let existing = root.join("org--model").join("Model.gguf");
        std::fs::write(&existing, b"GGUF trunc").expect("truncated destination");
        assert!(
            move_model_into_library(&root, &source.to_string_lossy(), Some("org/model")).is_err(),
            "legacy hf_browser move deleted the source over any existing destination"
        );
        assert_eq!(
            std::fs::read(&source).expect("source kept"),
            b"GGUF full model"
        );
        assert_eq!(
            std::fs::read(&existing).expect("destination"),
            b"GGUF trunc"
        );
        std::fs::write(&existing, b"GGUF full model").expect("identical destination");
        assert_eq!(
            move_model_into_library(&root, &source.to_string_lossy(), Some("org/model")),
            Ok(existing.to_string_lossy().into_owned())
        );
        assert!(!source.exists());
        let copy_source = app.join("Copy.gguf");
        std::fs::write(&copy_source, b"GGUF copy").expect("copy source");
        let copy_target = root.join("Copy.gguf");
        copy_verified(&copy_source, &copy_target).expect("verified copy");
        assert_eq!(std::fs::read(&copy_target).expect("copied"), b"GGUF copy");
        assert!(copy_verified(&copy_source, &copy_target).is_err());
        let other = app.join("Other.gguf");
        std::fs::write(&other, b"GGUF other").expect("other");
        assert_eq!(
            publish_no_clobber(&other, &copy_target).map_err(|error| error.kind()),
            Err(std::io::ErrorKind::AlreadyExists)
        );
        assert_eq!(std::fs::read(&copy_target).expect("kept"), b"GGUF copy");
        assert!(other.exists());
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
    #[test]
    fn rebound_manifest_journals_finish_or_undo_without_removing_changed_files() {
        for committed in [false, true] {
            for tampered in [false, true] {
                let app = scratch("rebound-journal");
                let from = app.join("source");
                let to = app.join("destination");
                let relative = PathBuf::from("model/manifest.json");
                std::fs::create_dir_all(from.join("model")).expect("source");
                std::fs::create_dir_all(to.join("model")).expect("target");
                let before = serde_json::to_vec(&serde_json::json!({"path":from.join("model/weights")})).expect("before");
                let after = serde_json::to_vec(&serde_json::json!({"path":to.join("model/weights")})).expect("after");
                std::fs::write(from.join(&relative), &before).expect("original manifest");
                std::fs::write(to.join(&relative), &after).expect("rebound manifest");
                std::fs::write(from.join("model/weights"), b"weights").expect("original weights");
                std::fs::write(to.join("model/weights"), b"weights").expect("copied weights");
                let manifest = MoveManifest { move_id: "rebind".into(), from: from.to_string_lossy().into_owned(),
                    entries: vec![ManifestEntry { name: "model".into(), measure: measure(&from.join("model")).expect("measure") }],
                    rebound_files: vec![ReboundManifest { relative_path: relative.clone(),
                        before_hash: blake3::hash(&before).to_hex().to_string(), after_hash: blake3::hash(&after).to_hex().to_string() }],
                };
                std::fs::write(to.join(MODELS_MOVE_MANIFEST), serde_json::to_vec(&manifest).expect("json")).expect("journal");
                if tampered { std::fs::write(to.join("model/weights"), b"changed").expect("external change"); }
                let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
                let database = backend.database();
                let mut device = database.load_device_settings().expect("device");
                device.llm_models_dir = Some(if committed { &to } else { &from }.to_string_lossy().into_owned());
                database.save_device_settings(device).expect("chosen folder");
                let result = recover_models_folder_move(database, &app, &to, "rebind").expect("recover").expect("journal");
                assert_eq!(result.committed, committed);
                assert_eq!(result.kept.is_empty(), !tampered);
                if tampered {
                    assert!(from.join("model").exists()); assert!(to.join("model").exists());
                } else if committed { assert!(!from.join("model").exists()); assert!(to.join("model").exists()); }
                else { assert!(from.join("model").exists()); assert!(!to.join("model").exists()); }
                std::fs::remove_dir_all(app).expect("cleanup");
            }
        }
    }

    #[test]
    fn manifest_rebinding_proof_rejects_content_identity_changes() {
        let before = br#"{"model":{"path":"/source/weights","blake3":"abc","byte_size":7},"revision":"fixed"}"#;
        let valid = br#"{"model":{"path":"/target/weights","blake3":"abc","byte_size":7},"revision":"fixed"}"#;
        let changed = br#"{"model":{"path":"/target/weights","blake3":"def","byte_size":7},"revision":"fixed"}"#;
        assert!(paths_only_rebinding(before, valid, Path::new("/source"), Path::new("/target")));
        assert!(!paths_only_rebinding(before, changed, Path::new("/source"), Path::new("/target")));
    }

}
