use base64::{engine::general_purpose::STANDARD, Engine};
use std::fs;
use std::path::PathBuf;

use crate::storage_manager::legacy::storage_root;

const TTS_AUDIO_DIR: &str = "tts_audio";

pub fn generate_cache_key(
    provider_id: &str,
    model_id: &str,
    voice_id: &str,
    text: &str,
    prompt: Option<&str>,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(provider_id.as_bytes());
    hasher.update(b"|");
    hasher.update(model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(voice_id.as_bytes());
    hasher.update(b"|");
    hasher.update(text.as_bytes());
    hasher.update(b"|");
    if let Some(p) = prompt {
        hasher.update(p.as_bytes());
    }
    let result = hasher.finalize();
    result.to_hex().to_string()
}

/// Get the directory for TTS audio cache
fn tts_audio_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let root = storage_root(app)?;
    let dir = root.join(TTS_AUDIO_DIR);
    fs::create_dir_all(&dir).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create TTS audio directory: {}", e),
        )
    })?;
    Ok(dir)
}

fn format_to_extension(format: &str) -> &str {
    match format {
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/wav" | "audio/wave" => "wav",
        "audio/ogg" => "ogg",
        "audio/webm" => "webm",
        _ => "audio",
    }
}

fn extension_to_format(ext: &str) -> String {
    match ext {
        "mp3" => "audio/mpeg".to_string(),
        "wav" => "audio/wav".to_string(),
        "ogg" => "audio/ogg".to_string(),
        "webm" => "audio/webm".to_string(),
        _ => "audio/octet-stream".to_string(),
    }
}

pub fn save_audio_to_cache(
    app: &tauri::AppHandle,
    cache_key: &str,
    audio_data: &[u8],
    format: &str,
) -> Result<(), String> {
    let dir = tts_audio_dir(app)?;
    let ext = format_to_extension(format);
    let file_path = dir.join(format!("{}.{}", cache_key, ext));
    fs::write(&file_path, audio_data).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to save TTS audio to cache: {}", e),
        )
    })?;
    Ok(())
}

pub fn load_audio_from_cache(
    app: &tauri::AppHandle,
    cache_key: &str,
) -> Result<Option<(Vec<u8>, String)>, String> {
    let dir = tts_audio_dir(app)?;

    for ext in &["mp3", "wav", "ogg", "webm", "audio"] {
        let file_path = dir.join(format!("{}.{}", cache_key, ext));
        if file_path.exists() {
            let audio_data = fs::read(&file_path).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to read TTS audio from cache: {}", e),
                )
            })?;
            let format = extension_to_format(ext);
            return Ok(Some((audio_data, format)));
        }
    }

    Ok(None)
}

pub fn audio_exists_in_cache(app: &tauri::AppHandle, cache_key: &str) -> Result<bool, String> {
    let dir = tts_audio_dir(app)?;

    for ext in &["mp3", "wav", "ogg", "webm", "audio"] {
        let file_path = dir.join(format!("{}.{}", cache_key, ext));
        if file_path.exists() {
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn delete_audio_from_cache(app: &tauri::AppHandle, cache_key: &str) -> Result<(), String> {
    let dir = tts_audio_dir(app)?;

    for ext in &["mp3", "wav", "ogg", "webm", "audio"] {
        let file_path = dir.join(format!("{}.{}", cache_key, ext));
        if file_path.exists() {
            fs::remove_file(&file_path).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to delete TTS audio from cache: {}", e),
                )
            })?;
        }
    }

    Ok(())
}

pub fn clear_audio_cache(app: &tauri::AppHandle) -> Result<u64, String> {
    let dir = tts_audio_dir(app)?;
    let mut count = 0u64;

    if dir.exists() {
        for entry in fs::read_dir(&dir)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to read TTS cache directory: {}", e),
                )
            })?
            .flatten()
        {
            let path = entry.path();
            if path.is_file() && fs::remove_file(&path).is_ok() {
                count += 1;
            }
        }
    }

    Ok(count)
}

pub fn get_cache_size(app: &tauri::AppHandle) -> Result<u64, String> {
    let dir = tts_audio_dir(app)?;
    let mut total_size = 0u64;

    if dir.exists() {
        for entry in fs::read_dir(&dir)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to read TTS cache directory: {}", e),
                )
            })?
            .flatten()
        {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    total_size += metadata.len();
                }
            }
        }
    }

    Ok(total_size)
}

pub fn get_cache_count(app: &tauri::AppHandle) -> Result<u64, String> {
    let dir = tts_audio_dir(app)?;
    let mut count = 0u64;

    if dir.exists() {
        for entry in fs::read_dir(&dir)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to read TTS cache directory: {}", e),
                )
            })?
            .flatten()
        {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    count += 1;
                }
            }
        }
    }

    Ok(count)
}

#[tauri::command]
pub fn tts_cache_key(
    provider_id: String,
    model_id: String,
    voice_id: String,
    text: String,
    prompt: Option<String>,
) -> String {
    generate_cache_key(&provider_id, &model_id, &voice_id, &text, prompt.as_deref())
}

#[tauri::command]
pub fn tts_cache_exists(app: tauri::AppHandle, cache_key: String) -> Result<bool, String> {
    audio_exists_in_cache(&app, &cache_key)
}

#[tauri::command]
pub fn tts_cache_get(
    app: tauri::AppHandle,
    cache_key: String,
) -> Result<Option<super::types::TtsPreviewResponse>, String> {
    match load_audio_from_cache(&app, &cache_key)? {
        Some((audio_data, format)) => {
            let audio_base64 = STANDARD.encode(&audio_data);
            Ok(Some(super::types::TtsPreviewResponse {
                audio_base64,
                format,
            }))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub fn tts_cache_save(
    app: tauri::AppHandle,
    cache_key: String,
    audio_base64: String,
    format: String,
) -> Result<(), String> {
    let audio_data = STANDARD.decode(&audio_base64).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to decode audio base64: {}", e),
        )
    })?;
    save_audio_to_cache(&app, &cache_key, &audio_data, &format)
}

#[tauri::command]
pub fn tts_cache_delete(app: tauri::AppHandle, cache_key: String) -> Result<(), String> {
    delete_audio_from_cache(&app, &cache_key)
}

#[tauri::command]
pub fn tts_cache_clear(app: tauri::AppHandle) -> Result<u64, String> {
    clear_audio_cache(&app)
}

#[tauri::command]
pub fn tts_cache_stats(app: tauri::AppHandle) -> Result<TtsCacheStats, String> {
    let size_bytes = get_cache_size(&app)?;
    let count = get_cache_count(&app)?;
    Ok(TtsCacheStats { size_bytes, count })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsCacheStats {
    pub size_bytes: u64,
    pub count: u64,
}
