use base64::{engine::general_purpose, Engine as _};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::Manager;

use super::legacy::storage_root;
use crate::utils::{log_debug, log_info};

pub struct StoredImageInfo {
    pub file_path: String,
    pub mime_type: String,
}

fn validate_simple_id<'a>(value: &'a str, field: &str) -> Result<&'a str, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("{field} cannot be empty"),
        ));
    }

    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid {field}: {}", value),
        ));
    }

    Ok(trimmed)
}

fn validate_single_component<'a>(
    value: &'a str,
    field: &str,
    allow_dots: bool,
) -> Result<&'a str, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("{field} cannot be empty"),
        ));
    }

    let path = Path::new(trimmed);
    if path.is_absolute() || path.components().count() != 1 {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid {field}: {}", value),
        ));
    }

    match path.components().next() {
        Some(Component::Normal(_)) => {}
        _ => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid {field}: {}", value),
            ))
        }
    }

    if !trimmed.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') || (allow_dots && ch == '.')
    }) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid {field}: {}", value),
        ));
    }

    Ok(trimmed)
}

fn validate_avatar_filename<'a>(filename: &'a str) -> Result<&'a str, String> {
    let filename = validate_single_component(filename, "avatar filename", true)?;
    let path = PathBuf::from(filename);
    if !is_supported_image_file(&path) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unsupported avatar filename: {}", filename),
        ));
    }

    Ok(filename)
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageLibraryItem {
    pub id: String,
    pub bucket: String,
    pub file_path: String,
    pub storage_path: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub updated_at: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub variant: Option<String>,
    pub character_id: Option<String>,
    pub session_id: Option<String>,
    pub role: Option<String>,
}

fn decode_base64_payload(base64_data: &str) -> Result<Vec<u8>, String> {
    let data = if let Some(comma_idx) = base64_data.find(',') {
        &base64_data[comma_idx + 1..]
    } else {
        base64_data
    };

    general_purpose::STANDARD.decode(data).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to decode base64: {}", e),
        )
    })
}

fn image_extension_from_bytes(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if bytes.starts_with(&[0x47, 0x49, 0x46]) {
        "gif"
    } else if bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        "webp"
    } else {
        "png"
    }
}

fn image_mime_type_from_extension(extension: &str) -> &'static str {
    match extension {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

fn is_supported_image_file(path: &PathBuf) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "webp"
            )
        })
        .unwrap_or(false)
}

fn scan_image_dir(
    root: &PathBuf,
    current: &PathBuf,
    out: &mut Vec<ImageLibraryItem>,
) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(current)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let path = entry.path();
        if path.is_dir() {
            scan_image_dir(root, &path, out)?;
            continue;
        }
        if !is_supported_image_file(&path) {
            continue;
        }

        if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
            if file_name == "avatar_round.webp" {
                continue;
            }

            if file_name == "avatar.webp" {
                let sibling = path.with_file_name("avatar_base.webp");
                if sibling.exists() {
                    continue;
                }
            }
        }

        let storage_path = path
            .strip_prefix(root)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .to_string_lossy()
            .replace('\\', "/");
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        let metadata = fs::metadata(&path)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let updated_at = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        let (width, height) = image::image_dimensions(&path).ok().unwrap_or((0, 0));

        let mut item = ImageLibraryItem {
            id: storage_path.clone(),
            bucket: "stored".to_string(),
            file_path: path.to_string_lossy().to_string(),
            storage_path,
            filename,
            mime_type: image_mime_type_from_extension(
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or_default(),
            )
            .to_string(),
            size_bytes: metadata.len(),
            updated_at,
            width: if width > 0 { Some(width) } else { None },
            height: if height > 0 { Some(height) } else { None },
            entity_type: None,
            entity_id: None,
            variant: None,
            character_id: None,
            session_id: None,
            role: None,
        };

        if item.storage_path.starts_with("avatars/") {
            item.bucket = "avatar".to_string();
            let parts: Vec<&str> = item.storage_path.split('/').collect();
            if parts.len() >= 3 {
                let entity_dir = parts[1];
                if let Some(value) = entity_dir.strip_prefix("character-") {
                    item.entity_type = Some("character".to_string());
                    item.entity_id = Some(value.to_string());
                } else if let Some(value) = entity_dir.strip_prefix("persona-") {
                    item.entity_type = Some("persona".to_string());
                    item.entity_id = Some(value.to_string());
                }
                item.variant = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(|stem| stem.to_string());
            }
        } else if item.storage_path.starts_with("sessions/") {
            item.bucket = "attachment".to_string();
            let parts: Vec<&str> = item.storage_path.split('/').collect();
            if parts.len() >= 4 {
                item.character_id = Some(parts[1].to_string());
                item.session_id = Some(parts[2].to_string());
                let filename = parts[3];
                item.role = if filename.starts_with("ai_") {
                    Some("assistant".to_string())
                } else if filename.starts_with("user_") {
                    Some("user".to_string())
                } else {
                    None
                };
            }
        }

        out.push(item);
    }

    Ok(())
}

fn images_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let images_dir = storage_root(app)?.join("images");
    fs::create_dir_all(&images_dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(images_dir)
}

fn managed_media_roots(app: &tauri::AppHandle) -> Result<Vec<PathBuf>, String> {
    let mut roots = vec![storage_root(app)?];
    roots.push(
        app.path()
            .app_data_dir()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .join("generated_images"),
    );
    Ok(roots)
}

fn resolve_managed_media_source_path(
    app: &tauri::AppHandle,
    file_path: &str,
) -> Result<PathBuf, String> {
    let source_path = PathBuf::from(file_path);
    if !source_path.exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Image file not found: {}", file_path),
        ));
    }

    let canonical_source = fs::canonicalize(&source_path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for root in managed_media_roots(app)? {
        if root.exists() {
            let canonical_root = fs::canonicalize(&root)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            if canonical_source.starts_with(&canonical_root) {
                return Ok(canonical_source);
            }
        }
    }

    Err(crate::utils::err_msg(
        module_path!(),
        line!(),
        format!("Refusing to access unmanaged file path: {}", file_path),
    ))
}

fn is_safe_library_storage_path(storage_path: &str) -> bool {
    let path = PathBuf::from(storage_path);
    if path.is_absolute() {
        return false;
    }

    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return false;
    };

    let bucket = first.to_string_lossy();
    if !matches!(bucket.as_ref(), "images" | "avatars" | "sessions") {
        return false;
    }

    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn prune_empty_parent_dirs(mut current: PathBuf, root: &PathBuf) {
    while current.starts_with(root) && current != *root {
        let is_empty = fs::read_dir(&current)
            .ok()
            .and_then(|mut entries| entries.next())
            .is_none();

        if !is_empty {
            break;
        }

        if fs::remove_dir(&current).is_err() {
            break;
        }

        let Some(parent) = current.parent() else {
            break;
        };
        current = parent.to_path_buf();
    }
}

#[cfg(target_os = "android")]
fn downloads_dir(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let download_dir = PathBuf::from("/storage/emulated/0/Download");
    if !download_dir.exists() {
        fs::create_dir_all(&download_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    Ok(download_dir)
}

#[cfg(not(target_os = "android"))]
fn downloads_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let download_dir = app.path().download_dir().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to get downloads directory: {}", e),
        )
    })?;

    if !download_dir.exists() {
        fs::create_dir_all(&download_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    Ok(download_dir)
}

fn find_image_path(images_dir: &PathBuf, image_id: &str) -> Option<(PathBuf, &'static str)> {
    for ext in &["jpg", "jpeg", "png", "gif", "webp"] {
        let image_path = images_dir.join(format!("{}.{}", image_id, ext));
        if image_path.exists() {
            return Some((image_path, image_mime_type_from_extension(ext)));
        }
    }

    None
}

pub fn storage_write_image_bytes(
    app: &tauri::AppHandle,
    image_id: &str,
    bytes: &[u8],
) -> Result<StoredImageInfo, String> {
    let image_id = validate_simple_id(image_id, "image ID")?;
    let images_dir = images_dir(app)?;
    let extension = image_extension_from_bytes(bytes);
    let image_path = images_dir.join(format!("{}.{}", image_id, extension));
    fs::write(&image_path, bytes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(StoredImageInfo {
        file_path: image_path.to_string_lossy().to_string(),
        mime_type: image_mime_type_from_extension(extension).to_string(),
    })
}

pub fn storage_write_image_data(
    app: &tauri::AppHandle,
    image_id: &str,
    base64_data: &str,
) -> Result<StoredImageInfo, String> {
    let bytes = decode_base64_payload(base64_data)?;
    storage_write_image_bytes(app, image_id, &bytes)
}

pub fn storage_read_image_data(app: &tauri::AppHandle, image_id: &str) -> Result<String, String> {
    let image_id = validate_simple_id(image_id, "image ID")?;
    let images_dir = images_dir(app)?;
    if let Some((image_path, mime_type)) = find_image_path(&images_dir, image_id) {
        let bytes = fs::read(&image_path)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let base64_data = general_purpose::STANDARD.encode(&bytes);
        return Ok(format!("data:{};base64,{}", mime_type, base64_data));
    }

    Err(crate::utils::err_msg(
        module_path!(),
        line!(),
        format!("Image not found: {}", image_id),
    ))
}

#[tauri::command]
pub fn storage_write_image(
    app: tauri::AppHandle,
    image_id: String,
    base64_data: String,
) -> Result<String, String> {
    Ok(storage_write_image_data(&app, &image_id, &base64_data)?.file_path)
}

#[tauri::command]
pub fn storage_list_image_library(app: tauri::AppHandle) -> Result<Vec<ImageLibraryItem>, String> {
    let root = storage_root(&app)?;
    let mut items = Vec::new();

    for dir in ["images", "avatars", "sessions"] {
        let path = root.join(dir);
        scan_image_dir(&root, &path, &mut items)?;
    }

    items.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.storage_path.cmp(&b.storage_path))
    });

    Ok(items)
}

#[tauri::command]
pub fn storage_download_image_to_downloads(
    app: tauri::AppHandle,
    file_path: String,
    filename: Option<String>,
) -> Result<String, String> {
    let source_path = resolve_managed_media_source_path(&app, &file_path)?;

    let resolved_filename = filename
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .or_else(|| {
            source_path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
        })
        .ok_or_else(|| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                "Unable to resolve filename for image download".to_string(),
            )
        })?;
    let resolved_filename =
        validate_single_component(&resolved_filename, "download filename", true)?.to_string();

    let target_path = downloads_dir(&app)?.join(&resolved_filename);
    log_info(
        &app,
        "storage_download_image_to_downloads",
        format!(
            "Copying image to downloads: {} -> {}",
            file_path,
            target_path.display()
        ),
    );

    fs::copy(&source_path, &target_path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let saved_path = target_path
        .to_str()
        .ok_or_else(|| {
            crate::utils::err_msg(module_path!(), line!(), "Invalid download path".to_string())
        })?
        .to_string();

    log_info(
        &app,
        "storage_download_image_to_downloads",
        format!("Image copied to downloads: {}", saved_path),
    );

    Ok(saved_path)
}

#[tauri::command]
pub fn storage_get_image_path(app: tauri::AppHandle, image_id: String) -> Result<String, String> {
    let image_id = validate_simple_id(&image_id, "image ID")?;
    let images_dir = images_dir(&app)?;
    if let Some((image_path, _)) = find_image_path(&images_dir, image_id) {
        return Ok(image_path.to_string_lossy().to_string());
    }

    Err(crate::utils::err_msg(
        module_path!(),
        line!(),
        format!("Image not found: {}", image_id),
    ))
}

#[tauri::command]
pub fn storage_delete_image(app: tauri::AppHandle, image_id: String) -> Result<(), String> {
    let image_id = validate_simple_id(&image_id, "image ID")?;
    let images_dir = images_dir(&app)?;
    for ext in &["jpg", "jpeg", "png", "gif", "webp", "img"] {
        let image_path = images_dir.join(format!("{}.{}", image_id, ext));
        if image_path.exists() {
            fs::remove_file(&image_path)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn storage_delete_image_library_item(
    app: tauri::AppHandle,
    storage_path: String,
) -> Result<(), String> {
    if !is_safe_library_storage_path(&storage_path) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unsafe image library path: {}", storage_path),
        ));
    }

    let root = storage_root(&app)?;
    let full_path = root.join(&storage_path);

    if !full_path.exists() {
        return Ok(());
    }

    if !is_supported_image_file(&full_path) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unsupported image library file: {}", storage_path),
        ));
    }

    fs::remove_file(&full_path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(parent) = full_path.parent() {
        prune_empty_parent_dirs(parent.to_path_buf(), &root);
    }

    Ok(())
}

#[tauri::command]
pub fn storage_read_image(app: tauri::AppHandle, image_id: String) -> Result<String, String> {
    storage_read_image_data(&app, &image_id)
}

#[tauri::command]
pub fn storage_save_avatar(
    app: tauri::AppHandle,
    entity_id: String,
    base64_data: String,
    round_base64_data: Option<String>,
) -> Result<String, String> {
    let entity_id = validate_simple_id(&entity_id, "entity ID")?;
    let data = if let Some(comma_idx) = base64_data.find(',') {
        &base64_data[comma_idx + 1..]
    } else {
        &base64_data
    };
    let bytes = general_purpose::STANDARD.decode(data).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to decode base64: {}", e),
        )
    })?;
    let avatars_dir = storage_root(&app)?.join("avatars").join(entity_id);
    fs::create_dir_all(&avatars_dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let base_webp_bytes = match image::load_from_memory(&bytes) {
        Ok(img) => {
            let mut webp_data: Vec<u8> = Vec::new();
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut webp_data);
            img.write_with_encoder(encoder).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to encode WebP: {}", e),
                )
            })?;
            webp_data
        }
        Err(_) => bytes,
    };
    let base_filename = "avatar_base.webp";
    let base_path = avatars_dir.join(base_filename);
    fs::write(&base_path, &base_webp_bytes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let legacy_path = avatars_dir.join("avatar.webp");
    fs::write(&legacy_path, &base_webp_bytes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let round_bytes = if let Some(round_data) = round_base64_data {
        let round_payload = if let Some(comma_idx) = round_data.find(',') {
            round_data[comma_idx + 1..].to_string()
        } else {
            round_data
        };
        general_purpose::STANDARD
            .decode(round_payload)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to decode round avatar base64: {}", e),
                )
            })?
    } else {
        base_webp_bytes.clone()
    };
    let round_webp_bytes = match image::load_from_memory(&round_bytes) {
        Ok(img) => {
            let mut webp_data: Vec<u8> = Vec::new();
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut webp_data);
            img.write_with_encoder(encoder).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to encode WebP: {}", e),
                )
            })?;
            webp_data
        }
        Err(_) => round_bytes,
    };
    let round_filename = "avatar_round.webp";
    let round_path = avatars_dir.join(round_filename);
    fs::write(&round_path, round_webp_bytes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let gradient_cache_path = avatars_dir.join("gradient.json");
    if gradient_cache_path.exists() {
        let _ = fs::remove_file(&gradient_cache_path);
        log_info(
            &app,
            "avatar",
            format!("Deleted gradient cache for {}", entity_id),
        );
    }
    Ok(base_filename.to_string())
}

#[tauri::command]
pub fn storage_load_avatar(
    app: tauri::AppHandle,
    entity_id: String,
    filename: String,
) -> Result<String, String> {
    let entity_id = validate_simple_id(&entity_id, "entity ID")?;
    let filename = validate_avatar_filename(&filename)?;
    let avatar_path = storage_root(&app)?
        .join("avatars")
        .join(entity_id)
        .join(filename);
    if !avatar_path.exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Avatar not found: {}/{}", entity_id, filename),
        ));
    }
    let bytes = fs::read(&avatar_path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mime_type = if filename.ends_with(".webp") {
        "image/webp"
    } else if filename.ends_with(".png") {
        "image/png"
    } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
        "image/jpeg"
    } else if filename.ends_with(".gif") {
        "image/gif"
    } else {
        "image/webp"
    };
    let base64_data = general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime_type, base64_data))
}

#[tauri::command]
pub fn storage_get_avatar_path(
    app: tauri::AppHandle,
    entity_id: String,
    filename: String,
) -> Result<String, String> {
    let entity_id = validate_simple_id(&entity_id, "entity ID")?;
    let filename = validate_avatar_filename(&filename)?;
    let avatar_path = storage_root(&app)?
        .join("avatars")
        .join(entity_id)
        .join(filename);
    if !avatar_path.exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Avatar not found: {}/{}", entity_id, filename),
        ));
    }
    Ok(avatar_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn storage_delete_avatar(
    app: tauri::AppHandle,
    entity_id: String,
    filename: String,
) -> Result<(), String> {
    let entity_id = validate_simple_id(&entity_id, "entity ID")?;
    let filename = validate_avatar_filename(&filename)?;
    let avatar_path = storage_root(&app)?
        .join("avatars")
        .join(entity_id)
        .join(filename);
    if avatar_path.exists() {
        fs::remove_file(&avatar_path)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct GradientColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hex: String,
}
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AvatarGradient {
    pub colors: Vec<GradientColor>,
    pub gradient_css: String,
    pub dominant_hue: f64,
    pub text_color: String,
    pub text_secondary: String,
}

#[tauri::command]
pub fn generate_avatar_gradient(
    app: tauri::AppHandle,
    entity_id: String,
    _filename: String,
) -> Result<AvatarGradient, String> {
    let entity_id = validate_simple_id(&entity_id, "entity ID")?;
    let avatars_dir = storage_root(&app)?.join("avatars").join(entity_id);
    let base_path = avatars_dir.join("avatar_base.webp");
    let legacy_path = avatars_dir.join("avatar.webp");
    let avatar_path = if base_path.exists() {
        base_path
    } else {
        legacy_path
    };
    let gradient_cache_path = avatars_dir.join("gradient.json");
    if !avatar_path.exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Avatar not found for {}", entity_id),
        ));
    }
    if gradient_cache_path.exists() {
        if let Ok(avatar_meta) = fs::metadata(&avatar_path) {
            if let Ok(cache_meta) = fs::metadata(&gradient_cache_path) {
                if let (Ok(avatar_time), Ok(cache_time)) =
                    (avatar_meta.modified(), cache_meta.modified())
                {
                    if cache_time >= avatar_time {
                        if let Ok(cached_json) = fs::read_to_string(&gradient_cache_path) {
                            if let Ok(cached_gradient) =
                                serde_json::from_str::<AvatarGradient>(&cached_json)
                            {
                                log_info(
                                    &app,
                                    "gradient",
                                    format!(
                                        "Using cached gradient from file for entity: {}",
                                        entity_id
                                    ),
                                );
                                return Ok(cached_gradient);
                            }
                        }
                    }
                }
            }
        }
    }
    log_info(
        &app,
        "gradient",
        format!("Processing avatar for entity: {}", entity_id),
    );
    let img = image::open(&avatar_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to load image: {}", e),
        )
    })?;
    let rgb_img = img.to_rgb8();
    let (width, height) = rgb_img.dimensions();
    log_debug(
        &app,
        "gradient",
        format!("Image dimensions: {}x{}", width, height),
    );
    let mut samples: Vec<(u8, u8, u8)> = Vec::new();
    let total_pixels = width * height;
    let target_samples = 100;
    let sample_step = ((total_pixels as f64 / target_samples as f64).sqrt()).max(1.0) as u32;
    for y in (0..height).step_by(sample_step as usize) {
        for x in (0..width).step_by(sample_step as usize) {
            if let Some(pixel) = rgb_img.get_pixel_checked(x, y) {
                let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
                let (_, s, v) = rgb_to_hsv(r, g, b);
                if v > 0.15 && v < 0.95 && s > 0.1 {
                    samples.push((r, g, b));
                }
            }
        }
    }
    if samples.is_empty() {
        return Ok(create_default_gradient());
    }
    let dominant_colors = find_dominant_colors(&samples, 3)?;
    let avg_hue = calculate_average_hue(&dominant_colors);
    let gradient_colors = generate_gradient_colors(&dominant_colors, avg_hue)?;
    let gradient_css = create_css_gradient(&gradient_colors);
    let (text_color, text_secondary) = calculate_text_colors(&gradient_colors);
    let gradient = AvatarGradient {
        colors: gradient_colors,
        gradient_css,
        dominant_hue: avg_hue,
        text_color,
        text_secondary,
    };
    if let Ok(json) = serde_json::to_string_pretty(&gradient) {
        let _ = fs::write(&gradient_cache_path, json);
    }
    Ok(gradient)
}

fn find_dominant_colors(samples: &[(u8, u8, u8)], k: usize) -> Result<Vec<(u8, u8, u8)>, String> {
    if samples.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No samples provided",
        ));
    }
    let mut centroids: Vec<(f64, f64, f64)> = Vec::new();
    let step = samples.len() / k.max(1);
    for i in 0..k {
        let idx = (i * step).min(samples.len() - 1);
        let sample = samples[idx];
        centroids.push((sample.0 as f64, sample.1 as f64, sample.2 as f64));
    }
    for _ in 0..8 {
        let mut clusters: Vec<Vec<(f64, f64, f64)>> = vec![Vec::new(); k];
        for &(r, g, b) in samples {
            let mut best = 0usize;
            let mut bestd = f64::MAX;
            for (i, &(cr, cg, cb)) in centroids.iter().enumerate() {
                let d = (r as f64 - cr).powi(2) + (g as f64 - cg).powi(2) + (b as f64 - cb).powi(2);
                if d < bestd {
                    bestd = d;
                    best = i;
                }
            }
            clusters[best].push((r as f64, g as f64, b as f64));
        }
        for (i, cluster) in clusters.iter().enumerate() {
            if !cluster.is_empty() {
                let (mut sr, mut sg, mut sb) = (0.0, 0.0, 0.0);
                for &(r, g, b) in cluster {
                    sr += r;
                    sg += g;
                    sb += b;
                }
                let l = cluster.len() as f64;
                centroids[i] = (sr / l, sg / l, sb / l);
            }
        }
    }
    Ok(centroids
        .into_iter()
        .map(|(r, g, b)| (r.round() as u8, g.round() as u8, b.round() as u8))
        .collect())
}

fn calculate_average_hue(colors: &[(u8, u8, u8)]) -> f64 {
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for &(r, g, b) in colors {
        let (h, s, v) = rgb_to_hsv(r, g, b);
        let weight = s * v;
        let angle = h.to_radians();
        sum_x += angle.cos() * weight;
        sum_y += angle.sin() * weight;
    }
    if sum_x == 0.0 && sum_y == 0.0 {
        0.0
    } else {
        sum_y.atan2(sum_x).to_degrees().rem_euclid(360.0)
    }
}

fn calculate_text_colors(colors: &[GradientColor]) -> (String, String) {
    let luminances: Vec<f64> = colors
        .iter()
        .map(|c| {
            0.2126 * (c.r as f64 / 255.0)
                + 0.7152 * (c.g as f64 / 255.0)
                + 0.0722 * (c.b as f64 / 255.0)
        })
        .collect();
    let avg = if luminances.is_empty() {
        0.0
    } else {
        luminances.iter().sum::<f64>() / luminances.len() as f64
    };
    if avg > 0.5 {
        ("#111827".into(), "#374151".into())
    } else {
        ("#F9FAFB".into(), "#D1D5DB".into())
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_avatar_filename, validate_simple_id, validate_single_component};

    #[test]
    fn validate_simple_id_rejects_traversal() {
        assert!(validate_simple_id("../escape", "id").is_err());
        assert!(validate_simple_id("nested/path", "id").is_err());
        assert!(validate_simple_id("", "id").is_err());
    }

    #[test]
    fn validate_avatar_filename_requires_single_supported_file() {
        assert!(validate_avatar_filename("avatar_base.webp").is_ok());
        assert!(validate_avatar_filename("../avatar_base.webp").is_err());
        assert!(validate_avatar_filename("nested/avatar_base.webp").is_err());
        assert!(validate_avatar_filename("avatar_base.txt").is_err());
    }

    #[test]
    fn validate_single_component_rejects_path_segments() {
        assert!(validate_single_component("../../x", "file", true).is_err());
        assert!(validate_single_component("/tmp/x", "file", true).is_err());
        assert!(validate_single_component("ok-name.png", "file", true).is_ok());
    }
}

fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let diff = max - min;
    let v = max;
    let s = if max == 0.0 { 0.0 } else { diff / max };
    let h = if diff == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / diff) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / diff + 2.0)
    } else {
        60.0 * ((r - g) / diff + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    (h, s, v)
}
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    )
}

fn generate_gradient_colors(
    colors: &[(u8, u8, u8)],
    _base_hue: f64,
) -> Result<Vec<GradientColor>, String> {
    let mut gradient_colors = Vec::new();
    for color in colors.iter() {
        let (h, s, v) = rgb_to_hsv(color.0, color.1, color.2);
        let boosted_s = (s * 1.2).min(0.85);
        let boosted_v = (v * 1.15).min(0.95);
        let (r, g, b) = hsv_to_rgb(h, boosted_s, boosted_v);
        let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
        gradient_colors.push(GradientColor { r, g, b, hex });
    }
    Ok(gradient_colors)
}

fn create_css_gradient(colors: &[GradientColor]) -> String {
    if colors.is_empty() {
        return "linear-gradient(135deg, #6366f1, #8b5cf6)".to_string();
    }
    let stops: Vec<String> = colors
        .iter()
        .enumerate()
        .map(|(i, color)| {
            let percent = (i as f64 / (colors.len() - 1) as f64) * 100.0;
            format!("{} {}%", color.hex, percent)
        })
        .collect();
    format!("linear-gradient(135deg, {})", stops.join(", "))
}

fn create_default_gradient() -> AvatarGradient {
    let colors = vec![
        GradientColor {
            r: 99,
            g: 102,
            b: 241,
            hex: "#6366f1".to_string(),
        },
        GradientColor {
            r: 139,
            g: 92,
            b: 246,
            hex: "#8b5cf6".to_string(),
        },
        GradientColor {
            r: 236,
            g: 72,
            b: 153,
            hex: "#ec4899".to_string(),
        },
    ];
    let gradient_css = "linear-gradient(135deg, #6366f1 0%, #8b5cf6 50%, #ec4899 100%)".to_string();
    AvatarGradient {
        colors,
        gradient_css,
        dominant_hue: 0.0,
        text_color: "#F9FAFB".into(),
        text_secondary: "#D1D5DB".into(),
    }
}

#[tauri::command]
pub fn storage_save_session_attachment(
    app: tauri::AppHandle,
    character_id: String,
    session_id: String,
    message_id: String,
    attachment_id: String,
    role: String, // "user" or "assistant"
    base64_data: String,
) -> Result<String, String> {
    let data = if let Some(comma_idx) = base64_data.find(',') {
        &base64_data[comma_idx + 1..]
    } else {
        &base64_data
    };

    let bytes = general_purpose::STANDARD.decode(data).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to decode base64: {}", e),
        )
    })?;

    let sessions_dir = storage_root(&app)?
        .join("sessions")
        .join(&character_id)
        .join(&session_id);
    fs::create_dir_all(&sessions_dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let role_prefix = if role == "assistant" { "ai" } else { "user" };

    let webp_bytes = match image::load_from_memory(&bytes) {
        Ok(img) => {
            let mut webp_data: Vec<u8> = Vec::new();
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut webp_data);
            img.write_with_encoder(encoder).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to encode WebP: {}", e),
                )
            })?;
            webp_data
        }
        Err(_) => bytes,
    };

    // Filename: <role>_<message_id>_<attachment_id>.webp
    let filename = format!("{}_{}_{}.webp", role_prefix, message_id, attachment_id);
    let image_path = sessions_dir.join(&filename);
    fs::write(&image_path, webp_bytes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let relative_path = format!("sessions/{}/{}/{}", character_id, session_id, filename);

    log_debug(
        &app,
        "session_attachment",
        format!("Saved attachment: {}", relative_path),
    );

    Ok(relative_path)
}

#[tauri::command]
pub fn storage_load_session_attachment(
    app: tauri::AppHandle,
    storage_path: String,
) -> Result<String, String> {
    let full_path = storage_root(&app)?.join(&storage_path);

    if !full_path.exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Attachment not found: {}", storage_path),
        ));
    }

    let bytes = fs::read(&full_path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Determine MIME type from extension
    let mime_type = if storage_path.ends_with(".webp") {
        "image/webp"
    } else if storage_path.ends_with(".png") {
        "image/png"
    } else if storage_path.ends_with(".jpg") || storage_path.ends_with(".jpeg") {
        "image/jpeg"
    } else if storage_path.ends_with(".gif") {
        "image/gif"
    } else {
        "image/webp"
    };

    let base64_data = general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime_type, base64_data))
}

#[tauri::command]
pub fn storage_get_session_attachment_path(
    app: tauri::AppHandle,
    storage_path: String,
) -> Result<String, String> {
    let full_path = storage_root(&app)?.join(&storage_path);
    if !full_path.exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Attachment not found: {}", storage_path),
        ));
    }
    Ok(full_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn storage_delete_session_attachments(
    app: tauri::AppHandle,
    character_id: String,
    session_id: String,
) -> Result<(), String> {
    let sessions_dir = storage_root(&app)?
        .join("sessions")
        .join(&character_id)
        .join(&session_id);

    if sessions_dir.exists() {
        fs::remove_dir_all(&sessions_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        log_info(
            &app,
            "session_attachment",
            format!(
                "Deleted all attachments for session: {}/{}",
                character_id, session_id
            ),
        );
    }

    Ok(())
}

#[tauri::command]
pub fn storage_session_attachment_exists(
    app: tauri::AppHandle,
    storage_path: String,
) -> Result<bool, String> {
    let full_path = storage_root(&app)?.join(&storage_path);
    Ok(full_path.exists())
}
