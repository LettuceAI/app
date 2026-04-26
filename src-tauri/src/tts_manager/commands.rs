use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::params;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::AppHandle;

use super::types::{
    AudioModel, AudioProvider, AudioProviderType, CachedVoice, CreatedVoiceResult,
    TtsPreviewResponse, UserVoice, VoiceDesignPreview,
};
use super::{elevenlabs, gemini, kokoro, openai_compatible};
use crate::abort_manager::AbortRegistry;
use crate::storage_manager::db::{now_ms, open_db};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroVoiceBlendInput {
    pub voice_id: String,
    pub weight: f32,
}

const SYSTEM_KOKORO_ID: &str = "system-kokoro";

fn ensure_default_kokoro_provider(
    conn: &rusqlite::Connection,
    app: &AppHandle,
) -> Result<(), String> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audio_providers WHERE id = ?",
            params![SYSTEM_KOKORO_ID],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if exists > 0 {
        return Ok(());
    }

    let asset_root = kokoro::default_asset_root(app)
        .ok()
        .map(|p| p.to_string_lossy().to_string());
    let variant = kokoro::kokoro_supported_model_variants()
        .into_iter()
        .next()
        .map(|v| v.id);

    let now = now_ms();
    let _ = conn.execute(
        "INSERT OR IGNORE INTO audio_providers
         (id, provider_type, label, api_key, project_id, location, base_url, request_path, kokoro_variant, asset_root, created_at, updated_at)
         VALUES (?1, 'kokoro', 'Kokoro (Local)', NULL, NULL, NULL, NULL, NULL, ?2, ?3, ?4, ?4)",
        params![SYSTEM_KOKORO_ID, variant, asset_root, now],
    );
    Ok(())
}

/// List all audio providers
#[tauri::command]
pub fn audio_provider_list(app: AppHandle) -> Result<Vec<AudioProvider>, String> {
    let conn = open_db(&app)?;
    let _ = ensure_default_kokoro_provider(&conn, &app);
    let mut stmt = conn
        .prepare(
            "SELECT id, provider_type, label, api_key, project_id, location, base_url, request_path, kokoro_variant, asset_root, created_at, updated_at
             FROM audio_providers ORDER BY created_at DESC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let providers = stmt
        .query_map([], |row| {
            Ok(AudioProvider {
                id: row.get(0)?,
                provider_type: row.get(1)?,
                label: row.get(2)?,
                api_key: row.get(3)?,
                project_id: row.get(4)?,
                location: row.get(5)?,
                base_url: row.get(6)?,
                request_path: row.get(7)?,
                kokoro_variant: row.get(8)?,
                asset_root: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(providers)
}

/// Create or update an audio provider
#[tauri::command]
pub fn audio_provider_upsert(
    app: AppHandle,
    provider_json: String,
) -> Result<AudioProvider, String> {
    let provider: AudioProvider = serde_json::from_str(&provider_json).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Invalid JSON: {}", e))
    })?;

    let conn = open_db(&app)?;
    let now = now_ms();

    let id = if provider.id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        provider.id.clone()
    };

    let location = provider
        .location
        .clone()
        .unwrap_or_else(|| "us-central1".to_string());
    let base_url = provider.base_url.clone();
    let request_path = provider.request_path.clone();

    let kokoro_variant = provider.kokoro_variant.clone();
    let asset_root = provider.asset_root.clone();

    conn.execute(
        "INSERT INTO audio_providers (id, provider_type, label, api_key, project_id, location, base_url, request_path, kokoro_variant, asset_root, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
         ON CONFLICT(id) DO UPDATE SET
            provider_type = excluded.provider_type,
            label = excluded.label,
            api_key = excluded.api_key,
            project_id = excluded.project_id,
            location = excluded.location,
            base_url = excluded.base_url,
            request_path = excluded.request_path,
            kokoro_variant = excluded.kokoro_variant,
            asset_root = excluded.asset_root,
            updated_at = excluded.updated_at",
        params![
            id,
            provider.provider_type,
            provider.label,
            provider.api_key,
            provider.project_id,
            location,
            base_url,
            request_path,
            kokoro_variant,
            asset_root,
            now
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(AudioProvider {
        id,
        provider_type: provider.provider_type,
        label: provider.label,
        api_key: provider.api_key,
        project_id: provider.project_id,
        location: provider.location,
        base_url: provider.base_url,
        request_path: provider.request_path,
        kokoro_variant: provider.kokoro_variant,
        asset_root: provider.asset_root,
        created_at: now,
        updated_at: now,
    })
}

/// Delete an audio provider
#[tauri::command]
pub fn audio_provider_delete(app: AppHandle, id: String) -> Result<(), String> {
    if id == SYSTEM_KOKORO_ID {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "The default Kokoro provider can't be deleted",
        ));
    }
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM audio_providers WHERE id = ?", params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

/// Get models for a provider type
fn kokoro_variants_as_models() -> Vec<AudioModel> {
    kokoro::kokoro_supported_model_variants()
        .into_iter()
        .map(|info| AudioModel {
            id: info.id.clone(),
            name: info.label,
            provider_type: "kokoro".to_string(),
        })
        .collect()
}

#[tauri::command]
pub fn audio_models_list(provider_type: String) -> Vec<AudioModel> {
    match AudioProviderType::from_str(&provider_type) {
        Some(AudioProviderType::GeminiTts) => gemini::get_models(),
        Some(AudioProviderType::Elevenlabs) => elevenlabs::get_models(),
        Some(AudioProviderType::OpenAiTts) => openai_compatible::default_models(),
        Some(AudioProviderType::Kokoro) => kokoro_variants_as_models(),
        None => vec![],
    }
}

#[tauri::command]
pub fn audio_voice_design_models_list(provider_type: String) -> Vec<AudioModel> {
    match AudioProviderType::from_str(&provider_type) {
        Some(AudioProviderType::GeminiTts) => gemini::get_models(),
        Some(AudioProviderType::Elevenlabs) => elevenlabs::get_voice_design_models(),
        Some(AudioProviderType::OpenAiTts) => openai_compatible::default_models(),
        Some(AudioProviderType::Kokoro) => kokoro_variants_as_models(),
        None => vec![],
    }
}

#[tauri::command]
pub fn kokoro_supported_variants() -> Vec<kokoro::KokoroModelVariantInfo> {
    kokoro::kokoro_supported_model_variants()
}

#[tauri::command]
pub fn kokoro_default_asset_root(app: AppHandle) -> Result<String, String> {
    kokoro::default_asset_root(&app)
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn kokoro_validate_assets(
    asset_root: String,
    variant: String,
    selected_voice_id: Option<String>,
) -> Result<kokoro::KokoroAssetStatus, String> {
    let variant = kokoro::KokoroModelVariant::from_str(&variant)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    kokoro::validate_assets(
        &PathBuf::from(asset_root),
        variant,
        selected_voice_id.as_deref(),
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn kokoro_list_installed_voices(
    asset_root: String,
) -> Result<Vec<kokoro::KokoroInstalledVoice>, String> {
    let status = kokoro::validate_assets(
        &PathBuf::from(asset_root),
        kokoro::KokoroModelVariant::Int8,
        None,
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(status.installed_voices)
}

#[tauri::command]
pub async fn kokoro_list_available_voices(
    asset_root: String,
) -> Result<Vec<kokoro::KokoroAvailableVoice>, String> {
    kokoro::list_available_voices(&PathBuf::from(asset_root))
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub async fn kokoro_install_model(
    app: AppHandle,
    asset_root: String,
    variant: String,
) -> Result<kokoro::KokoroQueuedInstall, String> {
    let variant = kokoro::KokoroModelVariant::from_str(&variant)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    kokoro::queue_model_install(app, PathBuf::from(asset_root), variant)
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub async fn kokoro_install_voice(
    app: AppHandle,
    asset_root: String,
    voice_id: String,
) -> Result<kokoro::KokoroQueuedInstall, String> {
    kokoro::queue_voice_install(app, PathBuf::from(asset_root), voice_id)
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub async fn kokoro_install_voices(
    app: AppHandle,
    asset_root: String,
    voice_ids: Vec<String>,
) -> Result<kokoro::KokoroQueuedInstall, String> {
    kokoro::queue_voices_install(app, PathBuf::from(asset_root), voice_ids)
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn kokoro_storage_stats(asset_root: String) -> kokoro::KokoroStorageStats {
    kokoro::storage_stats(&PathBuf::from(asset_root))
}

#[tauri::command]
pub fn kokoro_uninstall_model(asset_root: String, variant: String) -> Result<bool, String> {
    let variant = kokoro::KokoroModelVariant::from_str(&variant)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    kokoro::uninstall_model(&PathBuf::from(asset_root), variant)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn kokoro_uninstall_voice(asset_root: String, voice_id: String) -> Result<bool, String> {
    kokoro::uninstall_voice(&PathBuf::from(asset_root), &voice_id)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn kokoro_tokenize_preview(
    asset_root: String,
    voice_blend: Vec<KokoroVoiceBlendInput>,
    text: String,
    espeak_bin_path: Option<String>,
    espeak_data_path: Option<String>,
) -> Result<kokoro::KokoroTokenizePreview, String> {
    kokoro::preview_tokenization(
        &PathBuf::from(asset_root),
        &voice_blend
            .into_iter()
            .map(|voice| kokoro::KokoroVoiceBlendSpec {
                voice_id: voice.voice_id,
                weight: voice.weight,
            })
            .collect::<Vec<_>>(),
        &text,
        espeak_bin_path.map(PathBuf::from),
        espeak_data_path.map(PathBuf::from),
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub async fn kokoro_preview(
    asset_root: String,
    variant: String,
    voice_blend: Vec<KokoroVoiceBlendInput>,
    text: String,
    speed: Option<f32>,
    espeak_bin_path: Option<String>,
    espeak_data_path: Option<String>,
) -> Result<TtsPreviewResponse, String> {
    let variant = kokoro::KokoroModelVariant::from_str(&variant)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let request = kokoro::KokoroSynthesisRequest {
        asset_root: PathBuf::from(asset_root),
        variant,
        voice_blend: voice_blend
            .into_iter()
            .map(|voice| kokoro::KokoroVoiceBlendSpec {
                voice_id: voice.voice_id,
                weight: voice.weight,
            })
            .collect(),
        text,
        speed: speed.unwrap_or(1.0),
        espeak_bin_path: espeak_bin_path.map(PathBuf::from),
        espeak_data_path: espeak_data_path.map(PathBuf::from),
    };

    let audio_bytes =
        tokio::task::spawn_blocking(move || kokoro::engine::synthesize_to_wav(request))
            .await
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Kokoro preview task failed: {}", e),
                )
            })?
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(TtsPreviewResponse {
        audio_base64: STANDARD.encode(audio_bytes),
        format: "audio/wav".to_string(),
    })
}

/// Get cached voices for a provider
#[tauri::command]
pub fn audio_provider_voices(
    app: AppHandle,
    provider_id: String,
) -> Result<Vec<CachedVoice>, String> {
    let conn = open_db(&app)?;

    // First get the provider type and any provider-specific configuration we need
    let (provider_type, kokoro_variant, asset_root): (
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT provider_type, kokoro_variant, asset_root FROM audio_providers WHERE id = ?",
            params![provider_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Provider not found: {}", e),
            )
        })?;

    // For Gemini, return hardcoded voices
    if provider_type == "gemini_tts" {
        let now = now_ms();
        return Ok(gemini::get_voices()
            .into_iter()
            .map(|v| CachedVoice {
                id: format!("{}:{}", provider_id, v.voice_id),
                provider_id: provider_id.clone(),
                voice_id: v.voice_id,
                name: v.name,
                preview_url: v.preview_url,
                labels: v.labels,
                cached_at: now,
            })
            .collect());
    }

    if provider_type == "kokoro" {
        let asset_root = match asset_root {
            Some(root) if !root.trim().is_empty() => root,
            _ => return Ok(Vec::new()),
        };
        let variant = kokoro_variant
            .as_deref()
            .and_then(|value| kokoro::KokoroModelVariant::from_str(value).ok())
            .unwrap_or(kokoro::KokoroModelVariant::Int8);
        let status = kokoro::validate_assets(&PathBuf::from(asset_root), variant, None)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let now = now_ms();

        return Ok(status
            .installed_voices
            .into_iter()
            .map(|voice| CachedVoice {
                id: format!("{}:{}", provider_id, voice.id),
                provider_id: provider_id.clone(),
                voice_id: voice.id.clone(),
                name: voice.id,
                preview_url: None,
                labels: HashMap::from([
                    ("category".to_string(), "library".to_string()),
                    ("engine".to_string(), "kokoro".to_string()),
                ]),
                cached_at: now,
            })
            .collect());
    }

    if provider_type == "openai_tts" {
        return Ok(Vec::new());
    }

    // For ElevenLabs, return cached voices
    let mut stmt = conn
        .prepare(
            "SELECT id, provider_id, voice_id, name, preview_url, labels, cached_at
             FROM audio_voice_cache WHERE provider_id = ?",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let voices = stmt
        .query_map(params![provider_id], |row| {
            let labels_json: Option<String> = row.get(5)?;
            let labels: HashMap<String, String> = labels_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            Ok(CachedVoice {
                id: row.get(0)?,
                provider_id: row.get(1)?,
                voice_id: row.get(2)?,
                name: row.get(3)?,
                preview_url: row.get(4)?,
                labels,
                cached_at: row.get(6)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(voices)
}

/// Refresh voices from API (ElevenLabs only)
#[tauri::command]
pub async fn audio_provider_refresh_voices(
    app: AppHandle,
    provider_id: String,
) -> Result<Vec<CachedVoice>, String> {
    let conn = open_db(&app)?;

    // Get provider details
    let (provider_type, api_key): (String, Option<String>) = conn
        .query_row(
            "SELECT provider_type, api_key FROM audio_providers WHERE id = ?",
            params![provider_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Provider not found: {}", e),
            )
        })?;

    // Gemini uses hardcoded voices
    if provider_type == "gemini_tts" || provider_type == "kokoro" {
        return audio_provider_voices(app, provider_id);
    }

    if provider_type == "openai_tts" {
        return Ok(Vec::new());
    }

    // ElevenLabs - fetch from API
    let api_key = api_key.ok_or("API key not configured")?;
    let voices = elevenlabs::fetch_voices(&api_key, None).await?;

    // Clear old cache and insert new voices
    let now = now_ms();
    conn.execute(
        "DELETE FROM audio_voice_cache WHERE provider_id = ?",
        params![provider_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut cached = Vec::new();
    for v in voices {
        let id = format!("{}:{}", provider_id, v.voice_id);
        let labels_json = serde_json::to_string(&v.labels).unwrap_or_default();

        conn.execute(
            "INSERT INTO audio_voice_cache (id, provider_id, voice_id, name, preview_url, labels, cached_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![id, provider_id, v.voice_id, v.name, v.preview_url, labels_json, now],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        cached.push(CachedVoice {
            id,
            provider_id: provider_id.clone(),
            voice_id: v.voice_id,
            name: v.name,
            preview_url: v.preview_url,
            labels: v.labels,
            cached_at: now,
        });
    }

    Ok(cached)
}

/// List user voice configurations
#[tauri::command]
pub fn user_voice_list(app: AppHandle) -> Result<Vec<UserVoice>, String> {
    let conn = open_db(&app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, provider_id, name, model_id, voice_id, prompt, created_at, updated_at
             FROM user_voices ORDER BY created_at DESC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let voices = stmt
        .query_map([], |row| {
            Ok(UserVoice {
                id: row.get(0)?,
                provider_id: row.get(1)?,
                name: row.get(2)?,
                model_id: row.get(3)?,
                voice_id: row.get(4)?,
                prompt: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(voices)
}

/// Create or update a user voice configuration
#[tauri::command]
pub fn user_voice_upsert(app: AppHandle, voice_json: String) -> Result<UserVoice, String> {
    let voice: UserVoice = serde_json::from_str(&voice_json).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Invalid JSON: {}", e))
    })?;

    let conn = open_db(&app)?;
    let now = now_ms();

    let id = if voice.id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        voice.id.clone()
    };

    conn.execute(
        "INSERT INTO user_voices (id, provider_id, name, model_id, voice_id, prompt, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT(id) DO UPDATE SET
            provider_id = excluded.provider_id,
            name = excluded.name,
            model_id = excluded.model_id,
            voice_id = excluded.voice_id,
            prompt = excluded.prompt,
            updated_at = excluded.updated_at",
        params![
            id,
            voice.provider_id,
            voice.name,
            voice.model_id,
            voice.voice_id,
            voice.prompt,
            now
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(UserVoice {
        id,
        provider_id: voice.provider_id,
        name: voice.name,
        model_id: voice.model_id,
        voice_id: voice.voice_id,
        prompt: voice.prompt,
        created_at: now,
        updated_at: now,
    })
}

/// Delete a user voice configuration
#[tauri::command]
pub fn user_voice_delete(app: AppHandle, id: String) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM user_voices WHERE id = ?", params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

/// Generate TTS preview audio
#[tauri::command]
pub async fn tts_preview(
    app: AppHandle,
    provider_id: String,
    model_id: String,
    voice_id: String,
    prompt: Option<String>,
    text: String,
    request_id: Option<String>,
) -> Result<TtsPreviewResponse, String> {
    let conn = open_db(&app)?;

    // Get provider details
    let (provider_type, api_key, project_id, location, base_url, request_path, kokoro_variant, asset_root): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT provider_type, api_key, project_id, location, base_url, request_path, kokoro_variant, asset_root FROM audio_providers WHERE id = ?",
            params![provider_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Provider not found: {}", e),
            )
        })?;

    let api_key = if provider_type == "kokoro" {
        String::new()
    } else {
        api_key.ok_or("API key not configured")?
    };

    let mut abort_rx = request_id.as_ref().map(|id| {
        use tauri::Manager;
        let registry = app.state::<AbortRegistry>();
        registry.register(id.clone())
    });

    let generate_audio = async {
        match provider_type.as_str() {
            "gemini_tts" => {
                let project_id = project_id.ok_or("Project ID required for Gemini TTS")?;
                let data = gemini::generate_speech(
                    &text,
                    &voice_id,
                    &model_id,
                    prompt.as_deref(),
                    &api_key,
                    &project_id,
                    location.as_deref(),
                )
                .await?;
                Ok((data, "audio/wav".to_string()))
            }
            "elevenlabs" => {
                let data =
                    elevenlabs::generate_speech(&text, &voice_id, &model_id, &api_key).await?;
                Ok((data, "audio/mpeg".to_string()))
            }
            "openai_tts" => {
                let base_url = base_url.ok_or("Base URL required for OpenAI-compatible TTS")?;
                openai_compatible::generate_speech(
                    &base_url,
                    request_path.as_deref(),
                    &api_key,
                    &text,
                    &voice_id,
                    &model_id,
                    prompt.as_deref(),
                )
                .await
            }
            "kokoro" => {
                let asset_root = asset_root
                    .clone()
                    .ok_or("Asset root not configured for Kokoro provider")?;
                let variant_str = if !model_id.trim().is_empty() {
                    model_id.clone()
                } else {
                    kokoro_variant
                        .clone()
                        .ok_or("Kokoro model variant not configured")?
                };
                let variant = kokoro::KokoroModelVariant::from_str(&variant_str)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

                let blend: Vec<KokoroVoiceBlendInput> = if voice_id.trim().starts_with('[') {
                    serde_json::from_str(&voice_id).map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Invalid Kokoro voice blend JSON: {}", e),
                        )
                    })?
                } else if voice_id.trim().is_empty() {
                    return Err(crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        "No Kokoro voice configured",
                    ));
                } else {
                    vec![KokoroVoiceBlendInput {
                        voice_id: voice_id.trim().to_string(),
                        weight: 1.0,
                    }]
                };

                let blend_specs: Vec<kokoro::KokoroVoiceBlendSpec> = blend
                    .into_iter()
                    .filter(|entry| entry.weight > 0.0)
                    .map(|entry| kokoro::KokoroVoiceBlendSpec {
                        voice_id: entry.voice_id,
                        weight: entry.weight,
                    })
                    .collect();

                if blend_specs.is_empty() {
                    return Err(crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        "Kokoro voice blend has no positive weights",
                    ));
                }

                let speed = prompt
                    .as_ref()
                    .and_then(|p| serde_json::from_str::<serde_json::Value>(p).ok())
                    .and_then(|v| v.get("speed").and_then(|s| s.as_f64()))
                    .map(|f| f as f32)
                    .filter(|s| s.is_finite() && *s > 0.0)
                    .unwrap_or(1.0);

                let request = kokoro::KokoroSynthesisRequest {
                    asset_root: PathBuf::from(asset_root),
                    variant,
                    voice_blend: blend_specs,
                    text: text.clone(),
                    speed,
                    espeak_bin_path: None,
                    espeak_data_path: None,
                };

                let audio = tokio::task::spawn_blocking(move || {
                    kokoro::engine::synthesize_to_wav(request)
                })
                .await
                .map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Kokoro synthesis task failed: {}", e),
                    )
                })?
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

                Ok((audio, "audio/wav".to_string()))
            }
            _ => Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Unknown provider type: {}", provider_type),
            )),
        }
    };

    let result = if let Some(rx) = abort_rx.as_mut() {
        tokio::select! {
            _ = rx => {
                if let Some(id) = request_id.as_ref() {
                    use tauri::Manager;
                    let registry = app.state::<AbortRegistry>();
                    registry.unregister(id);
                }
                return Err(crate::utils::err_msg(module_path!(), line!(), "Request aborted by user"));
            }
            value = generate_audio => value,
        }
    } else {
        generate_audio.await
    };

    if let Some(id) = request_id.as_ref() {
        use tauri::Manager;
        let registry = app.state::<AbortRegistry>();
        registry.unregister(id);
    }

    let (audio_data, format) = result?;

    let audio_base64 = STANDARD.encode(&audio_data);

    Ok(TtsPreviewResponse {
        audio_base64,
        format,
    })
}

/// Verify API key for audio provider
#[tauri::command]
pub async fn audio_provider_verify(
    provider_type: String,
    api_key: String,
    project_id: Option<String>,
) -> Result<bool, String> {
    match provider_type.as_str() {
        "gemini_tts" => {
            let project_id = project_id.ok_or("Project ID required for Gemini TTS")?;
            gemini::verify_api_key(&api_key, &project_id).await
        }
        "elevenlabs" => elevenlabs::verify_api_key(&api_key).await,
        "openai_tts" => Ok(true),
        "kokoro" => Ok(true),
        _ => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unknown provider type: {}", provider_type),
        )),
    }
}

/// Search voices from ElevenLabs library
#[tauri::command]
pub async fn audio_provider_search_voices(
    app: AppHandle,
    provider_id: String,
    search: String,
) -> Result<Vec<CachedVoice>, String> {
    let conn = open_db(&app)?;

    let (provider_type, api_key): (String, Option<String>) = conn
        .query_row(
            "SELECT provider_type, api_key FROM audio_providers WHERE id = ?",
            params![provider_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Provider not found: {}", e),
            )
        })?;

    if provider_type == "openai_tts" {
        return Ok(Vec::new());
    }

    if provider_type != "elevenlabs" {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Voice search only available for ElevenLabs",
        ));
    }

    let api_key = api_key.ok_or("API key not configured")?;
    let voices = elevenlabs::fetch_voices(&api_key, Some(&search)).await?;

    let now = now_ms();
    Ok(voices
        .into_iter()
        .map(|v| CachedVoice {
            id: format!("{}:{}", provider_id, v.voice_id),
            provider_id: provider_id.clone(),
            voice_id: v.voice_id,
            name: v.name,
            preview_url: v.preview_url,
            labels: v.labels,
            cached_at: now,
        })
        .collect())
}

/// Design a voice from text description (ElevenLabs only)
#[tauri::command]
pub async fn voice_design_preview(
    app: AppHandle,
    provider_id: String,
    text_sample: String,
    voice_description: String,
    model_id: Option<String>,
    num_previews: Option<u32>,
) -> Result<Vec<VoiceDesignPreview>, String> {
    let conn = open_db(&app)?;

    let (provider_type, api_key): (String, Option<String>) = conn
        .query_row(
            "SELECT provider_type, api_key FROM audio_providers WHERE id = ?",
            params![provider_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Provider not found: {}", e),
            )
        })?;

    if provider_type != "elevenlabs" {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Voice design only available for ElevenLabs",
        ));
    }

    let api_key = api_key.ok_or("API key not configured")?;
    let previews = elevenlabs::design_voice(
        &api_key,
        &text_sample,
        &voice_description,
        model_id.as_deref(),
        num_previews,
    )
    .await?;

    Ok(previews
        .into_iter()
        .map(|p| VoiceDesignPreview {
            generated_voice_id: p.generated_voice_id,
            audio_base64: p.audio_base_64,
            duration_secs: p.duration_secs,
            media_type: p.media_type,
        })
        .collect())
}

/// Create and save a voice from a design preview (ElevenLabs only)
#[tauri::command]
pub async fn voice_design_create(
    app: AppHandle,
    provider_id: String,
    voice_name: String,
    generated_voice_id: String,
    voice_description: Option<String>,
) -> Result<CreatedVoiceResult, String> {
    let conn = open_db(&app)?;

    let (provider_type, api_key): (String, Option<String>) = conn
        .query_row(
            "SELECT provider_type, api_key FROM audio_providers WHERE id = ?",
            params![provider_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Provider not found: {}", e),
            )
        })?;

    if provider_type != "elevenlabs" {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Voice creation only available for ElevenLabs",
        ));
    }

    let api_key = api_key.ok_or("API key not configured")?;
    let result = elevenlabs::create_voice_from_preview(
        &api_key,
        &voice_name,
        &generated_voice_id,
        voice_description.as_deref(),
        None,
    )
    .await?;

    Ok(CreatedVoiceResult {
        voice_id: result.voice_id,
    })
}
