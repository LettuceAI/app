use base64::{engine::general_purpose, Engine as _};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::Manager;

use super::legacy::storage_root;
use crate::utils::{log_debug, log_info, log_warn};

pub struct StoredImageInfo {
    pub file_path: String,
    pub mime_type: String,
}

pub fn validate_simple_id<'a>(value: &'a str, field: &str) -> Result<&'a str, String> {
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

pub fn validate_single_component<'a>(
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

pub fn validate_avatar_filename(filename: &str) -> Result<&str, String> {
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
    pub group_key: String,
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

fn default_image_group_key(item: &ImageLibraryItem) -> String {
    item.storage_path
        .rsplit_once('.')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_else(|| item.storage_path.clone())
}

fn assign_duplicate_candidate_group_keys(items: &mut [ImageLibraryItem]) {
    use std::collections::HashMap;

    const UPDATED_AT_BUCKET_MS: i64 = 5_000;
    let mut candidate_groups: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, item) in items.iter().enumerate() {
        let width = item.width.unwrap_or(0);
        let height = item.height.unwrap_or(0);
        if width == 0 || height == 0 {
            continue;
        }

        let updated_bucket = if item.updated_at <= 0 {
            0
        } else {
            item.updated_at / UPDATED_AT_BUCKET_MS
        };
        let candidate_key = format!("{width}x{height}:{updated_bucket}");
        candidate_groups
            .entry(candidate_key)
            .or_default()
            .push(index);
    }

    for (_, indices) in candidate_groups {
        if indices.len() != 2 {
            continue;
        }

        let [first_index, second_index] = [indices[0], indices[1]];
        let first = &items[first_index];
        let second = &items[second_index];

        let first_ext = Path::new(&first.filename)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let second_ext = Path::new(&second.filename)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        let is_png_webp_pair = matches!(
            (first_ext.as_str(), second_ext.as_str()),
            ("png", "webp") | ("webp", "png")
        );
        if !is_png_webp_pair {
            continue;
        }

        let updated_bucket = if first.updated_at <= 0 {
            0
        } else {
            first.updated_at / UPDATED_AT_BUCKET_MS
        };
        let group_key = format!(
            "paired:{}x{}:{}",
            first.width.unwrap_or(0),
            first.height.unwrap_or(0),
            updated_bucket
        );

        items[first_index].group_key = group_key.clone();
        items[second_index].group_key = group_key;
    }
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
            if file_name == "avatar_round.webp" || file_name == "avatar_banner.webp" {
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
            group_key: String::new(),
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

        item.group_key = default_image_group_key(&item);

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

    assign_duplicate_candidate_group_keys(&mut items);

    items.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.storage_path.cmp(&b.storage_path))
    });

    Ok(items)
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AudioLibraryItem {
    pub id: String,
    pub storage_path: String,
    pub file_path: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub updated_at: i64,
    pub source: String,
    pub character_id: Option<String>,
    pub character_name: Option<String>,
    pub session_id: Option<String>,
    pub session_title: Option<String>,
    pub message_id: Option<String>,
    pub role: Option<String>,
}

fn audio_mime_type_from_extension(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "mp3" => "audio/mpeg",
        "wav" | "wave" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "aiff" | "aif" => "audio/aiff",
        "m4a" => "audio/mp4",
        "webm" => "audio/webm",
        "opus" => "audio/opus",
        _ => "audio/mpeg",
    }
}

fn is_supported_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "mp3"
                    | "wav"
                    | "wave"
                    | "ogg"
                    | "oga"
                    | "flac"
                    | "aac"
                    | "aiff"
                    | "aif"
                    | "m4a"
                    | "webm"
                    | "opus"
            )
        })
        .unwrap_or(false)
}

fn collect_uploaded_audio(
    app: &tauri::AppHandle,
    root: &Path,
    out: &mut Vec<AudioLibraryItem>,
) -> Result<(), String> {
    let conn = crate::storage_manager::db::open_db(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.session_id, m.created_at, m.role, m.attachments, s.character_id, s.title, c.name
             FROM messages m
             JOIN sessions s ON m.session_id = s.id
             LEFT JOIN characters c ON s.character_id = c.id
             WHERE m.attachments LIKE '%audio/%'",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for row in rows {
        let (
            message_id,
            session_id,
            created_at,
            role,
            attachments_json,
            character_id,
            session_title,
            char_name,
        ) = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let attachments: Vec<crate::chat_manager::types::ImageAttachment> =
            serde_json::from_str(&attachments_json).unwrap_or_default();

        for attachment in attachments {
            if !attachment.mime_type.starts_with("audio/") {
                continue;
            }
            let Some(storage_path) = attachment.storage_path.clone() else {
                continue;
            };
            let abs = root.join(&storage_path);
            if !abs.exists() {
                continue;
            }
            let size_bytes = fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);

            out.push(AudioLibraryItem {
                id: storage_path.clone(),
                file_path: abs.to_string_lossy().to_string(),
                storage_path,
                filename: attachment
                    .filename
                    .clone()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| "Audio".to_string()),
                mime_type: attachment.mime_type.clone(),
                size_bytes,
                updated_at: created_at,
                source: "upload".to_string(),
                character_id: character_id.clone(),
                character_name: char_name.clone(),
                session_id: Some(session_id.clone()),
                session_title: session_title.clone(),
                message_id: Some(message_id.clone()),
                role: Some(role.clone()),
            });
        }
    }

    Ok(())
}

fn scan_tts_audio(root: &Path, out: &mut Vec<AudioLibraryItem>) -> Result<(), String> {
    let dir = root.join("tts_audio");
    if !dir.exists() {
        return Ok(());
    }

    let entries =
        fs::read_dir(&dir).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let path = entry.path();
        if !path.is_file() || !is_supported_audio_file(&path) {
            continue;
        }

        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        let storage_path = format!("tts_audio/{}", filename);
        let metadata = fs::metadata(&path)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let updated_at = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);

        out.push(AudioLibraryItem {
            id: storage_path.clone(),
            file_path: path.to_string_lossy().to_string(),
            storage_path,
            filename,
            mime_type: audio_mime_type_from_extension(
                path.extension().and_then(|ext| ext.to_str()).unwrap_or(""),
            )
            .to_string(),
            size_bytes: metadata.len(),
            updated_at,
            source: "tts".to_string(),
            character_id: None,
            character_name: None,
            session_id: None,
            session_title: None,
            message_id: None,
            role: None,
        });
    }

    Ok(())
}

fn is_safe_audio_library_path(storage_path: &str) -> bool {
    let path = PathBuf::from(storage_path);
    if path.is_absolute() {
        return false;
    }
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return false;
    };
    let bucket = first.to_string_lossy();
    if !matches!(bucket.as_ref(), "sessions" | "tts_audio") {
        return false;
    }
    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn remove_audio_attachment_reference(
    app: &tauri::AppHandle,
    storage_path: &str,
) -> Result<(), String> {
    let conn = crate::storage_manager::db::open_db(app)?;
    let like = format!("%{}%", storage_path);
    let mut stmt = conn
        .prepare("SELECT id, attachments FROM messages WHERE attachments LIKE ?1")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([&like], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut updates: Vec<(String, String)> = Vec::new();
    for row in rows {
        let (id, attachments_json) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut attachments: Vec<crate::chat_manager::types::ImageAttachment> =
            serde_json::from_str(&attachments_json).unwrap_or_default();
        let before = attachments.len();
        attachments.retain(|a| a.storage_path.as_deref() != Some(storage_path));
        if attachments.len() != before {
            let new_json = serde_json::to_string(&attachments).unwrap_or_else(|_| "[]".to_string());
            updates.push((id, new_json));
        }
    }
    drop(stmt);

    for (id, json) in updates {
        conn.execute(
            "UPDATE messages SET attachments = ?1 WHERE id = ?2",
            rusqlite::params![json, id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    Ok(())
}

#[tauri::command]
pub fn storage_delete_audio_library_item(
    app: tauri::AppHandle,
    storage_path: String,
) -> Result<(), String> {
    if !is_safe_audio_library_path(&storage_path) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unsafe audio library path: {}", storage_path),
        ));
    }

    let root = storage_root(&app)?;
    let full_path = root.join(&storage_path);

    if full_path.exists() {
        if !is_supported_audio_file(&full_path) {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Unsupported audio library file: {}", storage_path),
            ));
        }
        fs::remove_file(&full_path)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    if storage_path.starts_with("sessions/") {
        if let Err(err) = remove_audio_attachment_reference(&app, &storage_path) {
            log_warn(
                &app,
                "audio_library",
                format!("failed to clean audio attachment reference: {}", err),
            );
        }
    }

    Ok(())
}

#[tauri::command]
pub fn storage_list_audio_library(app: tauri::AppHandle) -> Result<Vec<AudioLibraryItem>, String> {
    let root = storage_root(&app)?;
    let mut items = Vec::new();

    if let Err(err) = collect_uploaded_audio(&app, &root, &mut items) {
        log_warn(
            &app,
            "audio_library",
            format!("failed to collect uploaded audio: {}", err),
        );
    }

    scan_tts_audio(&root, &mut items)?;

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
                "Unable to resolve filename for image download",
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
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "Invalid download path"))?
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
    banner_base64_data: Option<String>,
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
    let banner_path = avatars_dir.join("avatar_banner.webp");
    if let Some(banner_data) = banner_base64_data {
        let banner_payload = if let Some(comma_idx) = banner_data.find(',') {
            banner_data[comma_idx + 1..].to_string()
        } else {
            banner_data
        };
        let banner_bytes = general_purpose::STANDARD
            .decode(banner_payload)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to decode banner avatar base64: {}", e),
                )
            })?;
        let banner_webp_bytes = match image::load_from_memory(&banner_bytes) {
            Ok(img) => {
                let mut webp_data: Vec<u8> = Vec::new();
                let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut webp_data);
                img.write_with_encoder(encoder).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to encode banner WebP: {}", e),
                    )
                })?;
                webp_data
            }
            Err(_) => banner_bytes,
        };
        fs::write(&banner_path, banner_webp_bytes)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    } else if banner_path.exists() {
        let _ = fs::remove_file(&banner_path);
    }
    for cache_name in ["gradient.json", "gradient-base.json", "gradient-round.json"] {
        let gradient_cache_path = avatars_dir.join(cache_name);
        if gradient_cache_path.exists() {
            let _ = fs::remove_file(&gradient_cache_path);
        }
    }
    log_info(
        &app,
        "avatar",
        format!("Deleted gradient cache for {}", entity_id),
    );
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

#[derive(Clone, Debug)]
struct ClusterColor {
    r: u8,
    g: u8,
    b: u8,
    count: usize,
}

const SIGBITS: usize = 5;
const RSHIFT: usize = 8 - SIGBITS;
const HISTOSIZE: usize = 1 << (3 * SIGBITS);

#[derive(Clone, Debug)]
struct VBox {
    r1: usize,
    r2: usize,
    g1: usize,
    g2: usize,
    b1: usize,
    b2: usize,
    count: usize,
}

#[derive(Clone, Copy)]
struct SwatchTarget {
    min_saturation: f64,
    target_saturation: f64,
    max_saturation: f64,
    min_luminance: f64,
    target_luminance: f64,
    max_luminance: f64,
    saturation_weight: f64,
    luminance_weight: f64,
    population_weight: f64,
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
    force: Option<bool>,
    source: Option<String>,
) -> Result<AvatarGradient, String> {
    let entity_id = validate_simple_id(&entity_id, "entity ID")?;
    let force = force.unwrap_or(false);
    let source = source.unwrap_or_else(|| "round".to_string());
    let avatars_dir = storage_root(&app)?.join("avatars").join(entity_id);
    let round_path = avatars_dir.join("avatar_round.webp");
    let base_path = avatars_dir.join("avatar_base.webp");
    let legacy_path = avatars_dir.join("avatar.webp");
    let avatar_path = match source.as_str() {
        "base" => {
            if base_path.exists() {
                base_path
            } else {
                legacy_path
            }
        }
        _ => {
            if round_path.exists() {
                round_path
            } else if base_path.exists() {
                base_path
            } else {
                legacy_path
            }
        }
    };
    let gradient_cache_path = avatars_dir.join(match source.as_str() {
        "base" => "gradient-base.json",
        _ => "gradient-round.json",
    });
    if !avatar_path.exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Avatar not found for {}", entity_id),
        ));
    }
    if !force && gradient_cache_path.exists() {
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
                samples.push((r, g, b));
            }
        }
    }
    if samples.is_empty() {
        return Ok(create_default_gradient());
    }
    let dominant_colors = match find_dominant_colors(&samples, 8) {
        Ok(colors) if !colors.is_empty() => colors,
        Ok(_) => {
            log_warn(
                &app,
                "gradient",
                format!(
                    "Dominant color extraction returned no colors for entity: {}. Falling back to default gradient.",
                    entity_id
                ),
            );
            let gradient = create_default_gradient();
            let json = serde_json::to_string_pretty(&gradient).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to serialize gradient cache: {}", e),
                )
            })?;
            fs::write(&gradient_cache_path, json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            return Ok(gradient);
        }
        Err(err) => {
            log_warn(
                &app,
                "gradient",
                format!(
                    "Gradient extraction failed for entity {}: {}. Falling back to default gradient.",
                    entity_id, err
                ),
            );
            let gradient = create_default_gradient();
            let json = serde_json::to_string_pretty(&gradient).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to serialize gradient cache: {}", e),
                )
            })?;
            fs::write(&gradient_cache_path, json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            return Ok(gradient);
        }
    };
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
    let json = serde_json::to_string_pretty(&gradient).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize gradient cache: {}", e),
        )
    })?;
    fs::write(&gradient_cache_path, json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(gradient)
}

fn find_dominant_colors(samples: &[(u8, u8, u8)], k: usize) -> Result<Vec<ClusterColor>, String> {
    if samples.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No samples provided",
        ));
    }
    let hist = build_histogram(samples);
    let mut boxes = vec![create_vbox(samples, &hist)?];

    while boxes.len() < k {
        let Some((index, _)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, vbox)| vbox.count > 0 && vbox_can_split(vbox))
            .max_by(|(_, a), (_, b)| {
                vbox_score(a)
                    .partial_cmp(&vbox_score(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        else {
            break;
        };

        let vbox = boxes.remove(index);
        let (left, right) = split_vbox(vbox, &hist)?;
        boxes.push(left);
        boxes.push(right);
    }

    let mut result: Vec<ClusterColor> = boxes
        .into_iter()
        .filter_map(|vbox| average_color(&vbox, &hist))
        .collect();
    result.sort_by(|a, b| b.count.cmp(&a.count));
    Ok(result)
}

fn calculate_average_hue(colors: &[ClusterColor]) -> f64 {
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for color in colors {
        let (h, s, v) = rgb_to_hsv(color.r, color.g, color.b);
        let weight = s * v * color.count as f64;
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

fn build_histogram(samples: &[(u8, u8, u8)]) -> Vec<usize> {
    let mut hist = vec![0usize; HISTOSIZE];
    for &(r, g, b) in samples {
        let index = color_index(r >> RSHIFT, g >> RSHIFT, b >> RSHIFT);
        hist[index] += 1;
    }
    hist
}

fn color_index(r: u8, g: u8, b: u8) -> usize {
    ((r as usize) << (2 * SIGBITS)) + ((g as usize) << SIGBITS) + b as usize
}

fn create_vbox(samples: &[(u8, u8, u8)], hist: &[usize]) -> Result<VBox, String> {
    let mut rmin = usize::MAX;
    let mut rmax = 0usize;
    let mut gmin = usize::MAX;
    let mut gmax = 0usize;
    let mut bmin = usize::MAX;
    let mut bmax = 0usize;

    for &(r, g, b) in samples {
        let r = (r >> RSHIFT) as usize;
        let g = (g >> RSHIFT) as usize;
        let b = (b >> RSHIFT) as usize;
        rmin = rmin.min(r);
        rmax = rmax.max(r);
        gmin = gmin.min(g);
        gmax = gmax.max(g);
        bmin = bmin.min(b);
        bmax = bmax.max(b);
    }

    if rmin == usize::MAX {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Failed to create vbox",
        ));
    }

    let mut vbox = VBox {
        r1: rmin,
        r2: rmax,
        g1: gmin,
        g2: gmax,
        b1: bmin,
        b2: bmax,
        count: 0,
    };
    vbox.count = vbox_population(&vbox, hist);
    Ok(vbox)
}

fn vbox_population(vbox: &VBox, hist: &[usize]) -> usize {
    let mut sum = 0usize;
    for r in vbox.r1..=vbox.r2 {
        for g in vbox.g1..=vbox.g2 {
            for b in vbox.b1..=vbox.b2 {
                sum += hist[color_index(r as u8, g as u8, b as u8)];
            }
        }
    }
    sum
}

fn vbox_volume(vbox: &VBox) -> usize {
    (vbox.r2 - vbox.r1 + 1) * (vbox.g2 - vbox.g1 + 1) * (vbox.b2 - vbox.b1 + 1)
}

fn vbox_score(vbox: &VBox) -> f64 {
    vbox.count as f64 * vbox_volume(vbox) as f64
}

fn vbox_can_split(vbox: &VBox) -> bool {
    vbox.r1 < vbox.r2 || vbox.g1 < vbox.g2 || vbox.b1 < vbox.b2
}

fn average_color(vbox: &VBox, hist: &[usize]) -> Option<ClusterColor> {
    let mut total = 0usize;
    let mut rsum = 0usize;
    let mut gsum = 0usize;
    let mut bsum = 0usize;

    for r in vbox.r1..=vbox.r2 {
        for g in vbox.g1..=vbox.g2 {
            for b in vbox.b1..=vbox.b2 {
                let count = hist[color_index(r as u8, g as u8, b as u8)];
                if count == 0 {
                    continue;
                }
                total += count;
                rsum += count * ((r << RSHIFT) + (1 << (RSHIFT - 1)));
                gsum += count * ((g << RSHIFT) + (1 << (RSHIFT - 1)));
                bsum += count * ((b << RSHIFT) + (1 << (RSHIFT - 1)));
            }
        }
    }

    if total == 0 {
        return None;
    }

    Some(ClusterColor {
        r: (rsum / total).min(255) as u8,
        g: (gsum / total).min(255) as u8,
        b: (bsum / total).min(255) as u8,
        count: total,
    })
}

fn split_vbox(vbox: VBox, hist: &[usize]) -> Result<(VBox, VBox), String> {
    let r_range = vbox.r2 - vbox.r1;
    let g_range = vbox.g2 - vbox.g1;
    let b_range = vbox.b2 - vbox.b1;

    let axis = if r_range >= g_range && r_range >= b_range {
        0usize
    } else if g_range >= r_range && g_range >= b_range {
        1usize
    } else {
        2usize
    };

    let mut partial_sum = Vec::new();
    let mut total = 0usize;

    let (start, end) = match axis {
        0 => (vbox.r1, vbox.r2),
        1 => (vbox.g1, vbox.g2),
        _ => (vbox.b1, vbox.b2),
    };

    for i in start..=end {
        let mut sum = 0usize;
        match axis {
            0 => {
                for g in vbox.g1..=vbox.g2 {
                    for b in vbox.b1..=vbox.b2 {
                        sum += hist[color_index(i as u8, g as u8, b as u8)];
                    }
                }
            }
            1 => {
                for r in vbox.r1..=vbox.r2 {
                    for b in vbox.b1..=vbox.b2 {
                        sum += hist[color_index(r as u8, i as u8, b as u8)];
                    }
                }
            }
            _ => {
                for r in vbox.r1..=vbox.r2 {
                    for g in vbox.g1..=vbox.g2 {
                        sum += hist[color_index(r as u8, g as u8, i as u8)];
                    }
                }
            }
        }
        total += sum;
        partial_sum.push((i, total));
    }

    if total == 0 {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Failed to split empty vbox",
        ));
    }

    let mid = total / 2;
    let split_at = partial_sum
        .iter()
        .find(|(_, running)| *running >= mid)
        .map(|(index, _)| *index)
        .unwrap_or(start);

    let mut left = vbox.clone();
    let mut right = vbox;

    match axis {
        0 => {
            left.r2 = split_at;
            right.r1 = (split_at + 1).min(right.r2);
        }
        1 => {
            left.g2 = split_at;
            right.g1 = (split_at + 1).min(right.g2);
        }
        _ => {
            left.b2 = split_at;
            right.b1 = (split_at + 1).min(right.b2);
        }
    }

    left.count = vbox_population(&left, hist);
    right.count = vbox_population(&right, hist);

    if left.count == 0 || right.count == 0 {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Failed to split vbox into populated boxes",
        ));
    }

    Ok((left, right))
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
#[allow(dead_code)]
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
    colors: &[ClusterColor],
    _base_hue: f64,
) -> Result<Vec<GradientColor>, String> {
    if colors.is_empty() {
        return Ok(Vec::new());
    }

    let total_population = colors.iter().map(|c| c.count).sum::<usize>().max(1);
    let max_population = colors.iter().map(|c| c.count).max().unwrap_or(1);

    let dark_muted_target = SwatchTarget {
        min_saturation: 0.0,
        target_saturation: 0.30,
        max_saturation: 0.45,
        min_luminance: 0.0,
        target_luminance: 0.26,
        max_luminance: 0.40,
        saturation_weight: 0.24,
        luminance_weight: 0.52,
        population_weight: 0.24,
    };
    let muted_target = SwatchTarget {
        min_saturation: 0.0,
        target_saturation: 0.30,
        max_saturation: 0.45,
        min_luminance: 0.30,
        target_luminance: 0.50,
        max_luminance: 0.70,
        saturation_weight: 0.30,
        luminance_weight: 0.30,
        population_weight: 0.40,
    };
    let dark_vibrant_target = SwatchTarget {
        min_saturation: 0.35,
        target_saturation: 0.80,
        max_saturation: 1.0,
        min_luminance: 0.0,
        target_luminance: 0.26,
        max_luminance: 0.45,
        saturation_weight: 0.35,
        luminance_weight: 0.35,
        population_weight: 0.30,
    };

    let base = select_swatch_for_target(colors, dark_muted_target, max_population)
        .or_else(|| select_swatch_for_target(colors, muted_target, max_population))
        .or_else(|| select_swatch_for_target(colors, dark_vibrant_target, max_population))
        .unwrap_or_else(|| colors[0].clone());

    let companion =
        select_distinct_swatch_for_target(colors, &[base.clone()], muted_target, max_population)
            .or_else(|| {
                select_distinct_swatch_for_target(
                    colors,
                    &[base.clone()],
                    dark_muted_target,
                    max_population,
                )
            });

    let accent = select_distinct_swatch_for_target(
        colors,
        &companion
            .as_ref()
            .map(|c| vec![base.clone(), c.clone()])
            .unwrap_or_else(|| vec![base.clone()]),
        dark_vibrant_target,
        max_population,
    );

    let mut selected: Vec<ClusterColor> = vec![base];
    if let Some(color) = companion {
        selected.push(color);
    }
    if let Some(color) = accent {
        selected.push(color);
    }

    if selected.len() == 1 && colors.len() > 1 {
        selected.push(colors[1].clone());
    }

    let _ = total_population;

    Ok(selected
        .into_iter()
        .map(|color| GradientColor {
            r: color.r,
            g: color.g,
            b: color.b,
            hex: format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b),
        })
        .collect())
}

fn perceived_luminance(r: u8, g: u8, b: u8) -> f64 {
    (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0
}

fn color_distance(a: &ClusterColor, b: &ClusterColor) -> f64 {
    let dr = a.r as f64 - b.r as f64;
    let dg = a.g as f64 - b.g as f64;
    let db = a.b as f64 - b.b as f64;
    (dr * dr + dg * dg + db * db).sqrt()
}

fn score_target(color: &ClusterColor, target: SwatchTarget, max_population: usize) -> Option<f64> {
    let (_, saturation, _) = rgb_to_hsv(color.r, color.g, color.b);
    let luminance = perceived_luminance(color.r, color.g, color.b);

    if saturation < target.min_saturation
        || saturation > target.max_saturation
        || luminance < target.min_luminance
        || luminance > target.max_luminance
    {
        return None;
    }

    let saturation_score = 1.0 - (saturation - target.target_saturation).abs();
    let luminance_score = 1.0 - (luminance - target.target_luminance).abs();
    let population_score = color.count as f64 / max_population.max(1) as f64;

    Some(
        saturation_score * target.saturation_weight
            + luminance_score * target.luminance_weight
            + population_score * target.population_weight,
    )
}

fn select_swatch_for_target(
    colors: &[ClusterColor],
    target: SwatchTarget,
    max_population: usize,
) -> Option<ClusterColor> {
    colors
        .iter()
        .max_by(|a, b| {
            score_target(a, target, max_population)
                .unwrap_or(f64::MIN)
                .partial_cmp(&score_target(b, target, max_population).unwrap_or(f64::MIN))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|entry| score_target(entry, target, max_population).map(|_| entry.clone()))
}

fn select_distinct_swatch_for_target(
    colors: &[ClusterColor],
    selected: &[ClusterColor],
    target: SwatchTarget,
    max_population: usize,
) -> Option<ClusterColor> {
    colors
        .iter()
        .filter(|entry| {
            selected
                .iter()
                .all(|chosen| color_distance(entry, chosen) > 34.0)
        })
        .max_by(|a, b| {
            score_target(a, target, max_population)
                .unwrap_or(f64::MIN)
                .partial_cmp(&score_target(b, target, max_population).unwrap_or(f64::MIN))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|entry| score_target(entry, target, max_population).map(|_| entry.clone()))
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

fn audio_extension_from_mime(mime: &str) -> &'static str {
    match mime {
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/ogg" | "audio/vorbis" => "ogg",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/aac" => "aac",
        "audio/aiff" | "audio/x-aiff" => "aiff",
        "audio/mp4" | "audio/x-m4a" | "audio/m4a" => "m4a",
        _ => "wav",
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

    let declared_mime = base64_data
        .strip_prefix("data:")
        .and_then(|rest| rest.split(";base64,").next())
        .map(|s| s.trim().to_ascii_lowercase());
    let is_audio = declared_mime
        .as_deref()
        .map(|m| m.starts_with("audio/"))
        .unwrap_or(false);

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

    if is_audio {
        let extension = audio_extension_from_mime(declared_mime.as_deref().unwrap_or(""));
        let filename = format!(
            "{}_{}_{}.{}",
            role_prefix, message_id, attachment_id, extension
        );
        let audio_path = sessions_dir.join(&filename);
        fs::write(&audio_path, &bytes)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let relative_path = format!("sessions/{}/{}/{}", character_id, session_id, filename);
        log_debug(
            &app,
            "session_attachment",
            format!("Saved audio attachment: {}", relative_path),
        );
        return Ok(relative_path);
    }

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
    } else if storage_path.ends_with(".wav") {
        "audio/wav"
    } else if storage_path.ends_with(".mp3") {
        "audio/mpeg"
    } else if storage_path.ends_with(".ogg") {
        "audio/ogg"
    } else if storage_path.ends_with(".flac") {
        "audio/flac"
    } else if storage_path.ends_with(".aac") {
        "audio/aac"
    } else if storage_path.ends_with(".aiff") {
        "audio/aiff"
    } else if storage_path.ends_with(".m4a") {
        "audio/mp4"
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
