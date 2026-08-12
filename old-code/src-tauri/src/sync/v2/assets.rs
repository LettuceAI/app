use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use tauri::Manager;
use uuid::Uuid;

use super::protocol::BlobDescriptor;

#[derive(Debug, Clone)]
pub struct LocalAsset {
    pub descriptor: BlobDescriptor,
    pub source_path: PathBuf,
}

pub fn collect_local_assets(
    app: &tauri::AppHandle,
) -> Result<BTreeMap<String, LocalAsset>, String> {
    let storage_root = crate::storage_manager::legacy::storage_root(app)?;
    let app_data_root = app.path().app_data_dir().map_err(|error| error.to_string())?;
    let mut by_hash = BTreeMap::new();
    for directory in ["avatars", "images", "sessions"] {
        collect_directory(
            &storage_root.join(directory),
            directory,
            &mut by_hash,
        )?;
    }
    collect_directory(
        &app_data_root.join("generated_images"),
        "generated_images",
        &mut by_hash,
    )?;
    Ok(by_hash)
}

pub fn blob_cache_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("sync-v2-blobs"))
}

pub fn destination_path(
    app: &tauri::AppHandle,
    relative_path: &str,
) -> Result<PathBuf, String> {
    validate_relative_path(relative_path)?;
    if relative_path.starts_with("generated_images/") {
        return Ok(app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?
            .join(relative_path));
    }
    Ok(crate::storage_manager::legacy::storage_root(app)?.join(relative_path))
}

pub fn destination_has_hash(
    app: &tauri::AppHandle,
    relative_path: &str,
    expected_hash: &str,
) -> Result<bool, String> {
    let path = destination_path(app, relative_path)?;
    if !path.is_file() {
        return Ok(false);
    }
    Ok(hash_file(&path)? == expected_hash)
}

pub fn materialize_blob(
    app: &tauri::AppHandle,
    cached_blob: &Path,
    relative_paths: &[String],
    expected_hash: &str,
) -> Result<(), String> {
    for relative_path in relative_paths {
        if destination_has_hash(app, relative_path, expected_hash)? {
            continue;
        }
        let destination = destination_path(app, relative_path)?;
        let parent = destination
            .parent()
            .ok_or_else(|| format!("Asset path has no parent: {relative_path}"))?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = parent.join(format!(".sync-{}.partial", Uuid::new_v4()));
        fs::copy(cached_blob, &temporary).map_err(|error| error.to_string())?;
        File::open(&temporary)
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())?;
        fs::rename(&temporary, &destination).map_err(|error| error.to_string())?;
        File::open(parent)
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn retain_remote_winners(
    descriptor: &mut BlobDescriptor,
    local_path_hashes: &BTreeMap<String, String>,
) {
    descriptor.relative_paths.retain(|relative_path| {
        local_path_hashes
            .get(relative_path)
            .map(|local_hash| descriptor.content_hash > *local_hash)
            .unwrap_or(true)
    });
}

fn collect_directory(
    directory: &Path,
    prefix: &str,
    by_hash: &mut BTreeMap<String, LocalAsset>,
) -> Result<(), String> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = entry
            .path()
            .symlink_metadata()
            .map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "Asset filename is not valid UTF-8".to_string())?;
        let relative_path = format!("{prefix}/{name}");
        if metadata.is_dir() {
            collect_directory(&entry.path(), &relative_path, by_hash)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        validate_relative_path(&relative_path)?;
        let content_hash = hash_file(&entry.path())?;
        let asset = by_hash
            .entry(content_hash.clone())
            .or_insert_with(|| LocalAsset {
                descriptor: BlobDescriptor {
                    content_hash,
                    size_bytes: metadata.len(),
                    relative_paths: Vec::new(),
                },
                source_path: entry.path(),
            });
        if asset.descriptor.size_bytes != metadata.len() {
            return Err(format!(
                "Asset hash {} has inconsistent sizes",
                asset.descriptor.content_hash
            ));
        }
        asset.descriptor.relative_paths.push(relative_path);
    }
    Ok(())
}

fn validate_relative_path(relative_path: &str) -> Result<(), String> {
    let path = Path::new(relative_path);
    if relative_path.is_empty()
        || path.is_absolute()
        || relative_path.contains('\\')
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!("Unsafe asset path: {relative_path}"));
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{retain_remote_winners, validate_relative_path};
    use crate::sync::v2::protocol::BlobDescriptor;

    #[test]
    fn asset_paths_reject_escape_attempts() {
        assert!(validate_relative_path("images/a.webp").is_ok());
        assert!(validate_relative_path("../app.db").is_err());
        assert!(validate_relative_path("/tmp/file").is_err());
        assert!(validate_relative_path("images\\file").is_err());
    }

    #[test]
    fn concurrent_file_edits_choose_the_same_content_on_both_devices() {
        let low_hash = "1".repeat(64);
        let high_hash = "f".repeat(64);
        let path = "images/avatar.webp".to_string();

        let mut high_remote = BlobDescriptor {
            content_hash: high_hash.clone(),
            size_bytes: 20,
            relative_paths: vec![path.clone()],
        };
        retain_remote_winners(
            &mut high_remote,
            &BTreeMap::from([(path.clone(), low_hash.clone())]),
        );
        assert_eq!(high_remote.relative_paths, vec![path.clone()]);

        let mut low_remote = BlobDescriptor {
            content_hash: low_hash,
            size_bytes: 10,
            relative_paths: vec![path.clone()],
        };
        retain_remote_winners(
            &mut low_remote,
            &BTreeMap::from([(path, high_hash)]),
        );
        assert!(low_remote.relative_paths.is_empty());
    }
}
