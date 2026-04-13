use base64::{engine::general_purpose, Engine as _};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tauri::{Emitter, Manager};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::db::open_db;
use super::legacy::storage_root;
use crate::utils::log_info;
#[cfg(target_os = "android")]
use tauri_plugin_android_fs::{AndroidFs, AndroidFsExt};
#[cfg(target_os = "android")]
use tauri_plugin_fs::FilePath;
#[cfg(target_os = "android")]
use url::Url;

fn open_backup_file(_app: &tauri::AppHandle, path: &str) -> Result<File, String> {
    #[cfg(target_os = "android")]
    {
        let api = _app.android_fs();

        let url = Url::parse(path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid URI '{}': {}", path, e),
            )
        })?;
        let file_path = FilePath::Url(url);

        api.open_file(&file_path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to open Android file: {}", e),
            )
        })
    }

    #[cfg(not(target_os = "android"))]
    {
        File::open(path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to open backup file: {}", e),
            )
        })
    }
}

const BACKUP_VERSION: u32 = 2;

#[derive(Serialize, Deserialize)]
struct BackupManifest {
    version: u32,
    created_at: u64,
    app_version: String,
    encrypted: bool,
    /// Salt used for key derivation (base64)
    salt: Option<String>,
    /// Nonce used for encryption (base64)
    nonce: Option<String>,
}

/// Derive encryption key from password using BLAKE3
fn derive_key_from_password(password: &str, salt: &[u8; 16]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(password.as_bytes());
    hasher.update(salt);
    hasher.update(b"lettuce_backup_key_v1");
    let hash = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(hash.as_bytes());
    key
}

/// Encrypt data using XChaCha20-Poly1305
fn encrypt_data(data: &[u8], key: &[u8; 32], nonce: &[u8; 24]) -> Result<Vec<u8>, String> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let xnonce: XNonce = (*nonce).into();
    cipher.encrypt(&xnonce, data).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Encryption failed: {}", e))
    })
}

/// Decrypt data using XChaCha20-Poly1305
fn decrypt_data(data: &[u8], key: &[u8; 32], nonce: &[u8; 24]) -> Result<Vec<u8>, String> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let xnonce: XNonce = (*nonce).into();
    cipher.decrypt(&xnonce, data).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Decryption failed: {}", e))
    })
}

/// Get the downloads directory path
fn get_downloads_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    {
        Ok(PathBuf::from("/storage/emulated/0/Download"))
    }

    #[cfg(not(target_os = "android"))]
    {
        dirs::download_dir().ok_or_else(|| "Could not find Downloads directory".to_string())
    }
}

fn archive_name_targets_media(name: &str) -> bool {
    let normalized = name.replace('\\', "/");
    normalized.starts_with("images/")
        || normalized.starts_with("avatars/")
        || normalized.starts_with("attachments/")
        || normalized.starts_with("sessions/")
        || normalized.starts_with("generated_images/")
}

fn require_encrypted_backup(manifest: &BackupManifest) -> Result<(), String> {
    if manifest.encrypted {
        Ok(())
    } else {
        Err("Unencrypted backups are no longer supported. Create a password-protected backup instead.".to_string())
    }
}

fn require_non_empty_password<'a>(
    password: Option<&'a str>,
    context: &str,
) -> Result<&'a str, String> {
    let Some(password) = password
        .map(str::trim)
        .filter(|password| !password.is_empty())
    else {
        return Err(format!("Password required for {}", context));
    };

    Ok(password)
}

fn sanitize_media_archive_name(
    name: &str,
    strip_encryption_suffix: bool,
) -> Result<PathBuf, String> {
    let normalized = name.replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    let candidate = if strip_encryption_suffix {
        trimmed
            .strip_suffix(".enc")
            .ok_or_else(|| format!("Invalid encrypted media path in backup: {}", name))?
    } else {
        trimmed
    };

    let path = Path::new(candidate);
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return Err(format!("Invalid media path in backup: {}", name));
    };

    let root = first.to_string_lossy();
    if !matches!(
        root.as_ref(),
        "images" | "avatars" | "attachments" | "sessions" | "generated_images"
    ) {
        return Err(format!("Unexpected media root in backup: {}", name));
    }

    if !components.all(|component| matches!(component, Component::Normal(_))) {
        return Err(format!("Unsafe media path in backup: {}", name));
    }

    Ok(path.to_path_buf())
}

// ============================================================================
// TABLE EXPORT FUNCTIONS - Export each table to JSON
// ============================================================================

fn export_meta(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare("SELECT key, value FROM meta ORDER BY key ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "key": r.get::<_, String>(0)?,
                "value": r.get::<_, Option<String>>(1)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn export_settings(app: &tauri::AppHandle) -> Result<JsonValue, String> {
    let conn = open_db(app)?;

    let row: Option<(
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
        Option<String>,
        i64,
        i64,
    )> = conn
        .query_row(
            "SELECT default_provider_credential_id, default_model_id, app_state,
                    advanced_model_settings, prompt_template_id, system_prompt,
                    migration_version, advanced_settings, created_at, updated_at
             FROM settings WHERE id = 1",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                ))
            },
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some((
        default_provider,
        default_model,
        app_state,
        advanced_model_settings,
        prompt_template,
        system_prompt,
        migration_version,
        advanced_settings,
        created_at,
        updated_at,
    )) = row
    {
        Ok(serde_json::json!({
            "default_provider_credential_id": default_provider,
            "default_model_id": default_model,
            "app_state": serde_json::from_str::<JsonValue>(&app_state).unwrap_or(serde_json::json!({})),
            "advanced_model_settings": advanced_model_settings.and_then(|s| serde_json::from_str::<JsonValue>(&s).ok()),
            "prompt_template_id": prompt_template,
            "system_prompt": system_prompt,
            "migration_version": migration_version,
            "advanced_settings": advanced_settings.and_then(|s| serde_json::from_str::<JsonValue>(&s).ok()),
            "created_at": created_at,
            "updated_at": updated_at,
        }))
    } else {
        Ok(serde_json::json!({}))
    }
}

fn export_provider_credentials(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare("SELECT id, provider_id, label, api_key_ref, api_key, base_url, default_model, headers, config FROM provider_credentials")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "provider_id": r.get::<_, String>(1)?,
                "label": r.get::<_, String>(2)?,
                "api_key_ref": r.get::<_, Option<String>>(3)?,
                "api_key": r.get::<_, Option<String>>(4)?,
                "base_url": r.get::<_, Option<String>>(5)?,
                "default_model": r.get::<_, Option<String>>(6)?,
                "headers": r.get::<_, Option<String>>(7)?,
                "config": r.get::<_, Option<String>>(8)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn export_models(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare("SELECT id, name, provider_id, provider_credential_id, provider_label, display_name, created_at, model_type, input_scopes, output_scopes, advanced_model_settings, prompt_template_id, system_prompt FROM models")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "name": r.get::<_, String>(1)?,
                "provider_id": r.get::<_, String>(2)?,
                "provider_credential_id": r.get::<_, Option<String>>(3)?,
                "provider_label": r.get::<_, String>(4)?,
                "display_name": r.get::<_, String>(5)?,
                "created_at": r.get::<_, i64>(6)?,
                "model_type": r.get::<_, Option<String>>(7)?,
                "input_scopes": r.get::<_, Option<String>>(8)?,
                "output_scopes": r.get::<_, Option<String>>(9)?,
                "advanced_model_settings": r.get::<_, Option<String>>(10)?,
                "prompt_template_id": r.get::<_, Option<String>>(11)?,
                "system_prompt": r.get::<_, Option<String>>(12)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn export_secrets(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare("SELECT service, account, value, created_at, updated_at FROM secrets")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "service": r.get::<_, String>(0)?,
                "account": r.get::<_, String>(1)?,
                "value": r.get::<_, String>(2)?,
                "created_at": r.get::<_, i64>(3)?,
                "updated_at": r.get::<_, i64>(4)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn export_prompt_templates(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare("SELECT id, name, prompt_type, content, entries, condense_prompt_entries, created_at, updated_at FROM prompt_templates")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |r| {
            let entries_str: String = r.get(4)?;
            let entries_value = serde_json::from_str::<JsonValue>(&entries_str)
                .unwrap_or_else(|_| JsonValue::Array(vec![]));

            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "name": r.get::<_, String>(1)?,
                "prompt_type": r.get::<_, String>(2)?,
                "content": r.get::<_, String>(3)?,
                "entries": entries_value,
                "condense_prompt_entries": r.get::<_, i64>(5)? != 0,
                "created_at": r.get::<_, i64>(6)?,
                "updated_at": r.get::<_, i64>(7)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn export_personas(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare("SELECT id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, is_default, created_at, updated_at FROM personas")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "title": r.get::<_, String>(1)?,
                "description": r.get::<_, String>(2)?,
                "nickname": r.get::<_, Option<String>>(3)?,
                "avatar_path": r.get::<_, Option<String>>(4)?,
                "avatar_crop_x": r.get::<_, Option<f64>>(5)?,
                "avatar_crop_y": r.get::<_, Option<f64>>(6)?,
                "avatar_crop_scale": r.get::<_, Option<f64>>(7)?,
                "design_description": r.get::<_, Option<String>>(8)?,
                "design_reference_image_ids": r.get::<_, Option<String>>(9)?,
                "is_default": r.get::<_, i64>(10)? != 0,
                "created_at": r.get::<_, i64>(11)?,
                "updated_at": r.get::<_, i64>(12)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn export_characters(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;

    // Get all characters
    let mut stmt = conn
        .prepare("SELECT id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, background_image_path, description, definition, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, fallback_model_id, memory_type, prompt_template_id, group_chat_prompt_template_id, group_chat_roleplay_prompt_template_id, system_prompt, voice_config, voice_autoplay, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, chat_appearance, default_chat_template_id, created_at, updated_at FROM characters")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let characters: Vec<(String, JsonValue)> = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let json = serde_json::json!({
                "id": id.clone(),
                "name": r.get::<_, String>(1)?,
                "avatar_path": r.get::<_, Option<String>>(2)?,
                "avatar_crop_x": r.get::<_, Option<f64>>(3)?,
                "avatar_crop_y": r.get::<_, Option<f64>>(4)?,
                "avatar_crop_scale": r.get::<_, Option<f64>>(5)?,
                "design_description": r.get::<_, Option<String>>(6)?,
                "design_reference_image_ids": r.get::<_, Option<String>>(7)?,
                "background_image_path": r.get::<_, Option<String>>(8)?,
                "description": r.get::<_, Option<String>>(9)?,
                "definition": r.get::<_, Option<String>>(10)?,
                "nickname": r.get::<_, Option<String>>(11)?,
                "scenario": r.get::<_, Option<String>>(12)?,
                "creator_notes": r.get::<_, Option<String>>(13)?,
                "creator": r.get::<_, Option<String>>(14)?,
                "creator_notes_multilingual": r.get::<_, Option<String>>(15)?,
                "source": r.get::<_, Option<String>>(16)?,
                "tags": r.get::<_, Option<String>>(17)?,
                "default_scene_id": r.get::<_, Option<String>>(18)?,
                "default_model_id": r.get::<_, Option<String>>(19)?,
                "fallback_model_id": r.get::<_, Option<String>>(20)?,
                "memory_type": r.get::<_, String>(21)?,
                "prompt_template_id": r.get::<_, Option<String>>(22)?,
                "group_chat_prompt_template_id": r.get::<_, Option<String>>(23)?,
                "group_chat_roleplay_prompt_template_id": r.get::<_, Option<String>>(24)?,
                "system_prompt": r.get::<_, Option<String>>(25)?,
                "voice_config": r.get::<_, Option<String>>(26)?,
                "voice_autoplay": r.get::<_, Option<i64>>(27)?.unwrap_or(0) != 0,
                "disable_avatar_gradient": r.get::<_, i64>(28)? != 0,
                "custom_gradient_enabled": r.get::<_, i64>(29)? != 0,
                "custom_gradient_colors": r.get::<_, Option<String>>(30)?,
                "custom_text_color": r.get::<_, Option<String>>(31)?,
                "custom_text_secondary": r.get::<_, Option<String>>(32)?,
                "chat_appearance": r.get::<_, Option<String>>(33)?,
                "default_chat_template_id": r.get::<_, Option<String>>(34)?,
                "created_at": r.get::<_, i64>(35)?,
                "updated_at": r.get::<_, i64>(36)?,
            });
            Ok((id, json))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // For each character, get rules and scenes
    let mut result = Vec::new();
    for (char_id, mut char_json) in characters {
        // Get rules
        let mut rules_stmt = conn
            .prepare("SELECT rule FROM character_rules WHERE character_id = ? ORDER BY idx")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rules: Vec<String> = rules_stmt
            .query_map([&char_id], |r| r.get(0))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        // Get scenes with variants
        let mut scenes_stmt = conn
            .prepare("SELECT id, content, direction, created_at, selected_variant_id FROM scenes WHERE character_id = ?")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let scenes: Vec<JsonValue> = scenes_stmt
            .query_map([&char_id], |r| {
                Ok(serde_json::json!({
                    "id": r.get::<_, String>(0)?,
                    "content": r.get::<_, String>(1)?,
                    "direction": r.get::<_, Option<String>>(2)?,
                    "created_at": r.get::<_, i64>(3)?,
                    "selected_variant_id": r.get::<_, Option<String>>(4)?,
                }))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        // Get scene variants for each scene
        let mut scenes_with_variants = Vec::new();
        for mut scene in scenes {
            let scene_id = scene["id"].as_str().unwrap_or("");
            let mut variants_stmt = conn
                .prepare("SELECT id, content, direction, created_at FROM scene_variants WHERE scene_id = ?")
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let variants: Vec<JsonValue> = variants_stmt
                .query_map([scene_id], |r| {
                    Ok(serde_json::json!({
                        "id": r.get::<_, String>(0)?,
                        "content": r.get::<_, String>(1)?,
                        "direction": r.get::<_, Option<String>>(2)?,
                        "created_at": r.get::<_, i64>(3)?,
                    }))
                })
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            scene["variants"] = serde_json::json!(variants);
            scenes_with_variants.push(scene);
        }

        char_json["rules"] = serde_json::json!(rules);
        char_json["scenes"] = serde_json::json!(scenes_with_variants);
        result.push(char_json);
    }

    Ok(result)
}

fn export_sessions(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;

    // Get all sessions
    let mut stmt = conn
        .prepare("SELECT id, character_id, title, background_image_path, system_prompt, selected_scene_id, persona_id, persona_disabled, voice_autoplay,
                         prompt_template_id, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k,
                         memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events,
                         memory_status, memory_error, memory_progress_step, archived, created_at, updated_at FROM sessions")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let sessions: Vec<(String, JsonValue)> = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let json = serde_json::json!({
                "id": id.clone(),
                "character_id": r.get::<_, String>(1)?,
                "title": r.get::<_, String>(2)?,
                "background_image_path": r.get::<_, Option<String>>(3)?,
                "system_prompt": r.get::<_, Option<String>>(4)?,
                "selected_scene_id": r.get::<_, Option<String>>(5)?,
                "persona_id": r.get::<_, Option<String>>(6)?,
                "persona_disabled": r.get::<_, i64>(7)? != 0,
                "voice_autoplay": r.get::<_, Option<i64>>(8)?.map(|value| value != 0),
                "prompt_template_id": r.get::<_, Option<String>>(9)?,
                "temperature": r.get::<_, Option<f64>>(10)?,
                "top_p": r.get::<_, Option<f64>>(11)?,
                "max_output_tokens": r.get::<_, Option<i64>>(12)?,
                "frequency_penalty": r.get::<_, Option<f64>>(13)?,
                "presence_penalty": r.get::<_, Option<f64>>(14)?,
                "top_k": r.get::<_, Option<i64>>(15)?,
                "memories": r.get::<_, String>(16)?,
                "memory_embeddings": r.get::<_, String>(17)?,
                "memory_summary": r.get::<_, Option<String>>(18)?,
                "memory_summary_token_count": r.get::<_, i64>(19)?,
                "memory_tool_events": r.get::<_, String>(20)?,
                "memory_status": r.get::<_, Option<String>>(21)?,
                "memory_error": r.get::<_, Option<String>>(22)?,
                "memory_progress_step": r.get::<_, Option<i64>>(23)?,
                "archived": r.get::<_, i64>(24)? != 0,
                "created_at": r.get::<_, i64>(25)?,
                "updated_at": r.get::<_, i64>(26)?,
            });
            Ok((id, json))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // For each session, get messages
    let mut result = Vec::new();
    for (session_id, mut session_json) in sessions {
        let mut messages_stmt = conn
            .prepare("SELECT id, role, content, created_at, prompt_tokens, completion_tokens, total_tokens,
                             selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning FROM messages
                      WHERE session_id = ? ORDER BY created_at ASC")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let messages: Vec<(String, JsonValue)> = messages_stmt
            .query_map([&session_id], |r| {
                let msg_id: String = r.get(0)?;
                let json = serde_json::json!({
                    "id": msg_id.clone(),
                    "role": r.get::<_, String>(1)?,
                    "content": r.get::<_, String>(2)?,
                    "created_at": r.get::<_, i64>(3)?,
                    "prompt_tokens": r.get::<_, Option<i64>>(4)?,
                    "completion_tokens": r.get::<_, Option<i64>>(5)?,
                    "total_tokens": r.get::<_, Option<i64>>(6)?,
                    "selected_variant_id": r.get::<_, Option<String>>(7)?,
                    "is_pinned": r.get::<_, i64>(8)? != 0,
                    "memory_refs": r.get::<_, String>(9)?,
                    "used_lorebook_entries": r.get::<_, String>(10)?,
                    "attachments": r.get::<_, String>(11)?,
                    "reasoning": r.get::<_, Option<String>>(12)?,
                });
                Ok((msg_id, json))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        // Get variants for each message
        let mut messages_with_variants = Vec::new();
        for (msg_id, mut msg_json) in messages {
            let mut variants_stmt = conn
                .prepare("SELECT id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning
                          FROM message_variants WHERE message_id = ?")
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            let variants: Vec<JsonValue> = variants_stmt
                .query_map([&msg_id], |r| {
                    Ok(serde_json::json!({
                        "id": r.get::<_, String>(0)?,
                        "content": r.get::<_, String>(1)?,
                        "created_at": r.get::<_, i64>(2)?,
                        "prompt_tokens": r.get::<_, Option<i64>>(3)?,
                        "completion_tokens": r.get::<_, Option<i64>>(4)?,
                        "total_tokens": r.get::<_, Option<i64>>(5)?,
                        "reasoning": r.get::<_, Option<String>>(6)?,
                    }))
                })
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            msg_json["variants"] = serde_json::json!(variants);
            messages_with_variants.push(msg_json);
        }

        session_json["messages"] = serde_json::json!(messages_with_variants);
        result.push(session_json);
    }

    Ok(result)
}

fn export_group_sessions(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived,
                    chat_type, starting_scene, background_image_path,
                    lorebook_ids, disable_character_lorebooks, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events,
                    memory_status, memory_error, memory_progress_step, speaker_selection_method, memory_type
             FROM group_sessions",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let sessions: Vec<(String, JsonValue)> = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let json = serde_json::json!({
                "id": id.clone(),
                "group_character_id": r.get::<_, Option<String>>(1)?,
                "name": r.get::<_, String>(2)?,
                "character_ids": r.get::<_, String>(3)?,
                "muted_character_ids": r.get::<_, String>(4)?,
                "persona_id": r.get::<_, Option<String>>(5)?,
                "created_at": r.get::<_, i64>(6)?,
                "updated_at": r.get::<_, i64>(7)?,
                "archived": r.get::<_, i64>(8)? != 0,
                "chat_type": r.get::<_, String>(9)?,
                "starting_scene": r.get::<_, Option<String>>(10)?,
                "background_image_path": r.get::<_, Option<String>>(11)?,
                "lorebook_ids": r.get::<_, String>(12)?,
                "disable_character_lorebooks": r.get::<_, i64>(13)? != 0,
                "memories": r.get::<_, String>(14)?,
                "memory_embeddings": r.get::<_, String>(15)?,
                "memory_summary": r.get::<_, String>(16)?,
                "memory_summary_token_count": r.get::<_, i64>(17)?,
                "memory_tool_events": r.get::<_, String>(18)?,
                "memory_status": r.get::<_, Option<String>>(19)?,
                "memory_error": r.get::<_, Option<String>>(20)?,
                "memory_progress_step": r.get::<_, Option<i64>>(21)?,
                "speaker_selection_method": r.get::<_, Option<String>>(22)?,
                "memory_type": r.get::<_, Option<String>>(23)?,
            });
            Ok((id, json))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut result = Vec::new();
    for (session_id, mut session_json) in sessions {
        let mut participation_stmt = conn
            .prepare(
                "SELECT id, character_id, speak_count, last_spoke_turn, last_spoke_at
                 FROM group_participation WHERE session_id = ?",
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let participation: Vec<JsonValue> = participation_stmt
            .query_map([&session_id], |r| {
                Ok(serde_json::json!({
                    "id": r.get::<_, String>(0)?,
                    "character_id": r.get::<_, String>(1)?,
                    "speak_count": r.get::<_, i64>(2)?,
                    "last_spoke_turn": r.get::<_, Option<i64>>(3)?,
                    "last_spoke_at": r.get::<_, Option<i64>>(4)?,
                }))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut messages_stmt = conn
            .prepare(
                "SELECT id, role, content, speaker_character_id, turn_number, created_at,
                        prompt_tokens, completion_tokens, total_tokens, selected_variant_id,
                        is_pinned, attachments, used_lorebook_entries, reasoning, selection_reasoning, model_id
                 FROM group_messages WHERE session_id = ? ORDER BY created_at ASC",
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let messages: Vec<(String, JsonValue)> = messages_stmt
            .query_map([&session_id], |r| {
                let msg_id: String = r.get(0)?;
                let json = serde_json::json!({
                    "id": msg_id.clone(),
                    "role": r.get::<_, String>(1)?,
                    "content": r.get::<_, String>(2)?,
                    "speaker_character_id": r.get::<_, Option<String>>(3)?,
                    "turn_number": r.get::<_, i64>(4)?,
                    "created_at": r.get::<_, i64>(5)?,
                    "prompt_tokens": r.get::<_, Option<i64>>(6)?,
                    "completion_tokens": r.get::<_, Option<i64>>(7)?,
                    "total_tokens": r.get::<_, Option<i64>>(8)?,
                    "selected_variant_id": r.get::<_, Option<String>>(9)?,
                    "is_pinned": r.get::<_, i64>(10)? != 0,
                    "attachments": r.get::<_, String>(11)?,
                    "used_lorebook_entries": r.get::<_, String>(12)?,
                    "reasoning": r.get::<_, Option<String>>(13)?,
                    "selection_reasoning": r.get::<_, Option<String>>(14)?,
                    "model_id": r.get::<_, Option<String>>(15)?,
                });
                Ok((msg_id, json))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut messages_with_variants = Vec::new();
        for (msg_id, mut msg_json) in messages {
            let mut variants_stmt = conn
                .prepare(
                    "SELECT id, content, speaker_character_id, created_at, prompt_tokens, completion_tokens,
                            total_tokens, reasoning, selection_reasoning, model_id
                     FROM group_message_variants WHERE message_id = ?",
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            let variants: Vec<JsonValue> = variants_stmt
                .query_map([&msg_id], |r| {
                    Ok(serde_json::json!({
                        "id": r.get::<_, String>(0)?,
                        "content": r.get::<_, String>(1)?,
                        "speaker_character_id": r.get::<_, Option<String>>(2)?,
                        "created_at": r.get::<_, i64>(3)?,
                        "prompt_tokens": r.get::<_, Option<i64>>(4)?,
                        "completion_tokens": r.get::<_, Option<i64>>(5)?,
                        "total_tokens": r.get::<_, Option<i64>>(6)?,
                        "reasoning": r.get::<_, Option<String>>(7)?,
                        "selection_reasoning": r.get::<_, Option<String>>(8)?,
                        "model_id": r.get::<_, Option<String>>(9)?,
                    }))
                })
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            msg_json["variants"] = serde_json::json!(variants);
            messages_with_variants.push(msg_json);
        }

        session_json["participation"] = serde_json::json!(participation);
        session_json["messages"] = serde_json::json!(messages_with_variants);
        result.push(session_json);
    }

    Ok(result)
}

fn export_usage_records(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;

    // Get all usage records
    let mut stmt = conn
        .prepare("SELECT id, timestamp, session_id, character_id, character_name, model_id, model_name,
                  provider_id, provider_label, operation_type, finish_reason, prompt_tokens, completion_tokens, total_tokens,
                  memory_tokens, summary_tokens, reasoning_tokens, image_tokens, prompt_cost, completion_cost, total_cost,
                  success, error_message FROM usage_records")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let records: Vec<(String, JsonValue)> = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let json = serde_json::json!({
                "id": id.clone(),
                "timestamp": r.get::<_, i64>(1)?,
                "session_id": r.get::<_, String>(2)?,
                "character_id": r.get::<_, String>(3)?,
                "character_name": r.get::<_, String>(4)?,
                "model_id": r.get::<_, String>(5)?,
                "model_name": r.get::<_, String>(6)?,
                "provider_id": r.get::<_, String>(7)?,
                "provider_label": r.get::<_, String>(8)?,
                "operation_type": r.get::<_, Option<String>>(9)?,
                "finish_reason": r.get::<_, Option<String>>(10)?,
                "prompt_tokens": r.get::<_, Option<i64>>(11)?,
                "completion_tokens": r.get::<_, Option<i64>>(12)?,
                "total_tokens": r.get::<_, Option<i64>>(13)?,
                "memory_tokens": r.get::<_, Option<i64>>(14)?,
                "summary_tokens": r.get::<_, Option<i64>>(15)?,
                "reasoning_tokens": r.get::<_, Option<i64>>(16)?,
                "image_tokens": r.get::<_, Option<i64>>(17)?,
                "prompt_cost": r.get::<_, Option<f64>>(18)?,
                "completion_cost": r.get::<_, Option<f64>>(19)?,
                "total_cost": r.get::<_, Option<f64>>(20)?,
                "success": r.get::<_, i64>(21)? != 0,
                "error_message": r.get::<_, Option<String>>(22)?,
            });
            Ok((id, json))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // For each record, get metadata
    let mut result = Vec::new();
    for (record_id, mut record_json) in records {
        let mut meta_stmt = conn
            .prepare("SELECT key, value FROM usage_metadata WHERE usage_id = ?")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let metadata: Vec<JsonValue> = meta_stmt
            .query_map([&record_id], |r| {
                Ok(serde_json::json!({
                    "key": r.get::<_, String>(0)?,
                    "value": r.get::<_, String>(1)?,
                }))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        record_json["metadata"] = serde_json::json!(metadata);
        result.push(record_json);
    }

    Ok(result)
}

fn export_lorebooks(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;

    let mut stmt = conn
        .prepare("SELECT id, name, avatar_path, keyword_detection_mode, created_at, updated_at FROM lorebooks")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let lorebooks: Vec<(String, JsonValue)> = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let json = serde_json::json!({
                "id": id.clone(),
                "name": r.get::<_, String>(1)?,
                "avatar_path": r.get::<_, Option<String>>(2)?,
                "keyword_detection_mode": r.get::<_, String>(3)?,
                "created_at": r.get::<_, i64>(4)?,
                "updated_at": r.get::<_, i64>(5)?,
            });
            Ok((id, json))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // For each lorebook, get its entries
    let mut result = Vec::new();
    for (lorebook_id, mut lorebook_json) in lorebooks {
        let mut entries_stmt = conn
            .prepare("SELECT id, title, enabled, always_active, keywords, case_sensitive, content, priority, display_order, created_at, updated_at FROM lorebook_entries WHERE lorebook_id = ? ORDER BY display_order ASC")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let entries: Vec<JsonValue> = entries_stmt
            .query_map([&lorebook_id], |r| {
                Ok(serde_json::json!({
                    "id": r.get::<_, String>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "enabled": r.get::<_, i64>(2)? != 0,
                    "always_active": r.get::<_, i64>(3)? != 0,
                    "keywords": r.get::<_, String>(4)?,
                    "case_sensitive": r.get::<_, i64>(5)? != 0,
                    "content": r.get::<_, String>(6)?,
                    "priority": r.get::<_, i64>(7)?,
                    "display_order": r.get::<_, i64>(8)?,
                    "created_at": r.get::<_, i64>(9)?,
                    "updated_at": r.get::<_, i64>(10)?,
                }))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        lorebook_json["entries"] = serde_json::json!(entries);
        result.push(lorebook_json);
    }

    Ok(result)
}

fn export_character_lorebooks(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;

    let mut stmt = conn
        .prepare("SELECT character_id, lorebook_id, enabled, display_order, created_at, updated_at FROM character_lorebooks")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let links: Vec<JsonValue> = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "character_id": r.get::<_, String>(0)?,
                "lorebook_id": r.get::<_, String>(1)?,
                "enabled": r.get::<_, i64>(2)? != 0,
                "display_order": r.get::<_, i64>(3)?,
                "created_at": r.get::<_, i64>(4)?,
                "updated_at": r.get::<_, i64>(5)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(links)
}

fn export_chat_templates(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, character_id, name, scene_id, prompt_template_id, created_at
             FROM chat_templates
             ORDER BY created_at ASC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let templates: Vec<(String, JsonValue)> = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            Ok((
                id.clone(),
                serde_json::json!({
                    "id": id,
                    "character_id": r.get::<_, String>(1)?,
                    "name": r.get::<_, String>(2)?,
                    "scene_id": r.get::<_, Option<String>>(3)?,
                    "prompt_template_id": r.get::<_, Option<String>>(4)?,
                    "created_at": r.get::<_, i64>(5)?,
                }),
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut result = Vec::new();
    for (template_id, mut template_json) in templates {
        let mut msg_stmt = conn
            .prepare(
                "SELECT id, idx, role, content
                 FROM chat_template_messages
                 WHERE template_id = ?
                 ORDER BY idx ASC",
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let messages: Vec<JsonValue> = msg_stmt
            .query_map([&template_id], |r| {
                Ok(serde_json::json!({
                    "id": r.get::<_, String>(0)?,
                    "idx": r.get::<_, i64>(1)?,
                    "role": r.get::<_, String>(2)?,
                    "content": r.get::<_, String>(3)?,
                }))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        template_json["messages"] = serde_json::json!(messages);
        result.push(template_json);
    }

    Ok(result)
}

fn export_creation_helper_sessions(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, creation_goal, status, session_json, uploaded_images_json, created_at, updated_at
             FROM creation_helper_sessions
             ORDER BY updated_at DESC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "creation_goal": r.get::<_, String>(1)?,
                "status": r.get::<_, String>(2)?,
                "session_json": r.get::<_, String>(3)?,
                "uploaded_images_json": r.get::<_, String>(4)?,
                "created_at": r.get::<_, i64>(5)?,
                "updated_at": r.get::<_, i64>(6)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn export_group_characters(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at,
                    archived, chat_type, starting_scene, background_image_path, lorebook_ids,
                    disable_character_lorebooks, speaker_selection_method, memory_type
             FROM group_characters
             ORDER BY updated_at DESC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "name": r.get::<_, String>(1)?,
                "character_ids": r.get::<_, String>(2)?,
                "muted_character_ids": r.get::<_, String>(3)?,
                "persona_id": r.get::<_, Option<String>>(4)?,
                "created_at": r.get::<_, i64>(5)?,
                "updated_at": r.get::<_, i64>(6)?,
                "archived": r.get::<_, i64>(7)? != 0,
                "chat_type": r.get::<_, String>(8)?,
                "starting_scene": r.get::<_, Option<String>>(9)?,
                "background_image_path": r.get::<_, Option<String>>(10)?,
                "lorebook_ids": r.get::<_, String>(11)?,
                "disable_character_lorebooks": r.get::<_, i64>(12)? != 0,
                "speaker_selection_method": r.get::<_, Option<String>>(13)?,
                "memory_type": r.get::<_, Option<String>>(14)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn export_model_pricing_cache(app: &tauri::AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT model_id, pricing_json, cached_at
             FROM model_pricing_cache
             ORDER BY cached_at DESC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "model_id": r.get::<_, String>(0)?,
                "pricing_json": r.get::<_, Option<String>>(1)?,
                "cached_at": r.get::<_, i64>(2)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

/// Export full app backup to a .lettuce file
#[tauri::command]
pub async fn backup_export(
    app: tauri::AppHandle,
    password: Option<String>,
) -> Result<String, String> {
    let storage = storage_root(&app)?;
    let images_dir = storage.join("images");
    let avatars_dir = storage.join("avatars");
    let attachments_dir = storage.join("attachments");
    let session_attachments_dir = storage.join("sessions");
    let generated_images_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .join("generated_images");

    // Generate timestamp for filename
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("lettuce_backup_{}.lettuce", timestamp);
    let downloads = get_downloads_dir()?;
    let output_path = downloads.join(&filename);

    log_info(
        &app,
        "backup",
        format!(
            "Starting backup export (v2 JSON format) to {:?}",
            output_path
        ),
    );

    let password = require_non_empty_password(
        password.as_deref(),
        "backup export. Unencrypted backups are no longer allowed",
    )?;
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    let key = derive_key_from_password(password, &salt);
    let encryption = Some((salt, nonce, key));

    // Create the zip file
    let file = File::create(&output_path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // Helper to add JSON data to zip (with optional encryption)
    let add_json_to_zip = |zip: &mut ZipWriter<File>,
                           name: &str,
                           data: &JsonValue,
                           enc: &Option<([u8; 16], [u8; 24], [u8; 32])>|
     -> Result<(), String> {
        let json_bytes = serde_json::to_string_pretty(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .into_bytes();
        if let Some((_, nonce, key)) = enc {
            let encrypted = encrypt_data(&json_bytes, key, nonce)?;
            zip.start_file(format!("data/{}.json.enc", name), options)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            zip.write_all(&encrypted)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        } else {
            zip.start_file(format!("data/{}.json", name), options)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            zip.write_all(&json_bytes)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
        Ok(())
    };

    // Export all tables to JSON
    log_info(&app, "backup", "Exporting metadata...");
    let meta = export_meta(&app)?;
    add_json_to_zip(&mut zip, "meta", &serde_json::json!(meta), &encryption)?;

    log_info(&app, "backup", "Exporting settings...");
    let settings = export_settings(&app)?;
    add_json_to_zip(&mut zip, "settings", &settings, &encryption)?;

    log_info(&app, "backup", "Exporting provider credentials...");
    let providers = export_provider_credentials(&app)?;
    add_json_to_zip(
        &mut zip,
        "provider_credentials",
        &serde_json::json!(providers),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting models...");
    let models = export_models(&app)?;
    add_json_to_zip(&mut zip, "models", &serde_json::json!(models), &encryption)?;

    log_info(&app, "backup", "Exporting model pricing cache...");
    let pricing_cache = export_model_pricing_cache(&app)?;
    add_json_to_zip(
        &mut zip,
        "model_pricing_cache",
        &serde_json::json!(pricing_cache),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting secrets...");
    let secrets = export_secrets(&app)?;
    add_json_to_zip(
        &mut zip,
        "secrets",
        &serde_json::json!(secrets),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting prompt templates...");
    let templates = export_prompt_templates(&app)?;
    add_json_to_zip(
        &mut zip,
        "prompt_templates",
        &serde_json::json!(templates),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting chat templates...");
    let chat_templates = export_chat_templates(&app)?;
    add_json_to_zip(
        &mut zip,
        "chat_templates",
        &serde_json::json!(chat_templates),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting personas...");
    let personas = export_personas(&app)?;
    add_json_to_zip(
        &mut zip,
        "personas",
        &serde_json::json!(personas),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting characters...");
    let characters = export_characters(&app)?;
    add_json_to_zip(
        &mut zip,
        "characters",
        &serde_json::json!(characters),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting sessions...");
    let sessions = export_sessions(&app)?;
    add_json_to_zip(
        &mut zip,
        "sessions",
        &serde_json::json!(sessions),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting group sessions...");
    let group_sessions = export_group_sessions(&app)?;
    add_json_to_zip(
        &mut zip,
        "group_sessions",
        &serde_json::json!(group_sessions),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting group character presets...");
    let group_characters = export_group_characters(&app)?;
    add_json_to_zip(
        &mut zip,
        "group_characters",
        &serde_json::json!(group_characters),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting usage records...");
    let usage_data = export_usage_records(&app)?;
    add_json_to_zip(
        &mut zip,
        "usage_records",
        &serde_json::json!(usage_data),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting lorebooks...");
    let lorebooks = export_lorebooks(&app)?;
    add_json_to_zip(
        &mut zip,
        "lorebooks",
        &serde_json::json!(lorebooks),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting character-lorebook links...");
    let char_lorebooks = export_character_lorebooks(&app)?;
    add_json_to_zip(
        &mut zip,
        "character_lorebooks",
        &serde_json::json!(char_lorebooks),
        &encryption,
    )?;

    log_info(&app, "backup", "Exporting creation helper sessions...");
    let creation_helper_sessions = export_creation_helper_sessions(&app)?;
    add_json_to_zip(
        &mut zip,
        "creation_helper_sessions",
        &serde_json::json!(creation_helper_sessions),
        &encryption,
    )?;

    // Add images directory
    if images_dir.exists() {
        add_directory_to_zip(
            &mut zip,
            &images_dir,
            "images",
            options,
            encryption.as_ref(),
        )?;
        log_info(&app, "backup", "Added images to archive");
    }

    // Add avatars directory
    if avatars_dir.exists() {
        add_directory_to_zip(
            &mut zip,
            &avatars_dir,
            "avatars",
            options,
            encryption.as_ref(),
        )?;
        log_info(&app, "backup", "Added avatars to archive");
    }

    // Add attachments directory
    if attachments_dir.exists() {
        add_directory_to_zip(
            &mut zip,
            &attachments_dir,
            "attachments",
            options,
            encryption.as_ref(),
        )?;
        log_info(&app, "backup", "Added attachments to archive");
    }

    if session_attachments_dir.exists() {
        add_directory_to_zip(
            &mut zip,
            &session_attachments_dir,
            "sessions",
            options,
            encryption.as_ref(),
        )?;
        log_info(&app, "backup", "Added session attachments to archive");
    }

    if generated_images_dir.exists() {
        add_directory_to_zip(
            &mut zip,
            &generated_images_dir,
            "generated_images",
            options,
            encryption.as_ref(),
        )?;
        log_info(&app, "backup", "Added generated images to archive");
    }

    // Create manifest
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let app_version = crate::utils::app_version(&app);

    let (salt, nonce, key) = encryption
        .as_ref()
        .expect("backup export encryption should always be configured");
    let marker = b"LETTUCE_BACKUP_VERIFIED";
    let encrypted_marker = encrypt_data(marker, key, nonce)?;

    zip.start_file("encrypted_marker.bin", options)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    zip.write_all(&encrypted_marker)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let manifest = BackupManifest {
        version: BACKUP_VERSION,
        created_at: now,
        app_version,
        encrypted: true,
        salt: Some(general_purpose::STANDARD.encode(salt)),
        nonce: Some(general_purpose::STANDARD.encode(nonce)),
    };

    // Add manifest
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    zip.start_file("manifest.json", options)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    zip.write_all(manifest_json.as_bytes())
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    zip.finish()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    log_info(
        &app,
        "backup",
        format!("Backup export complete: {:?}", output_path),
    );

    Ok(output_path.to_string_lossy().to_string())
}

/// Helper to add a directory recursively to zip (with optional encryption)
fn add_directory_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    dir: &PathBuf,
    prefix: &str,
    options: SimpleFileOptions,
    encryption: Option<&([u8; 16], [u8; 24], [u8; 32])>,
) -> Result<(), String> {
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            let relative = path
                .strip_prefix(dir)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                .to_string_lossy();

            let bytes = fs::read(path)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            if let Some((_, nonce, key)) = encryption {
                // Encrypt the file content
                let encrypted = encrypt_data(&bytes, key, nonce)?;
                let zip_path = format!("{}/{}.enc", prefix, relative);
                zip.start_file(&zip_path, options)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                zip.write_all(&encrypted)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            } else {
                let zip_path = format!("{}/{}", prefix, relative);
                zip.start_file(&zip_path, options)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                zip.write_all(&bytes)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }
        }
    }
    Ok(())
}

// ============================================================================
// TABLE IMPORT FUNCTIONS - Import each table from JSON
// ============================================================================

fn import_meta(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute("DELETE FROM meta", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            conn.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                params![
                    item.get("key").and_then(|v| v.as_str()),
                    item.get("value").and_then(|v| v.as_str()),
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }

    Ok(())
}

fn import_settings(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let app_state = data
        .get("app_state")
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string()))
        .unwrap_or_else(|| "{}".to_string());

    let advanced_settings = data.get("advanced_settings").and_then(|v| {
        if v.is_null() {
            None
        } else {
            Some(serde_json::to_string(v).ok()?)
        }
    });
    let advanced_model_settings = data.get("advanced_model_settings").and_then(|v| {
        if v.is_null() {
            None
        } else {
            Some(serde_json::to_string(v).ok()?)
        }
    });

    // Delete existing and insert new
    conn.execute("DELETE FROM settings", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        "INSERT INTO settings (id, default_provider_credential_id, default_model_id, app_state,
         advanced_model_settings, prompt_template_id, system_prompt, migration_version,
         advanced_settings, created_at, updated_at) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            data.get("default_provider_credential_id")
                .and_then(|v| v.as_str()),
            data.get("default_model_id").and_then(|v| v.as_str()),
            app_state,
            advanced_model_settings,
            data.get("prompt_template_id").and_then(|v| v.as_str()),
            data.get("system_prompt").and_then(|v| v.as_str()),
            data.get("migration_version")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            advanced_settings,
            data.get("created_at")
                .and_then(|v| v.as_i64())
                .unwrap_or(now),
            data.get("updated_at")
                .and_then(|v| v.as_i64())
                .or_else(|| data.get("created_at").and_then(|v| v.as_i64()))
                .unwrap_or(now),
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(())
}

fn disable_dynamic_memory_in_advanced_settings(settings: &mut serde_json::Value) {
    let Some(obj) = settings.as_object_mut() else {
        *settings = serde_json::json!({
            "dynamicMemory": {
                "enabled": false
            }
        });
        return;
    };

    match obj.get_mut("dynamicMemory") {
        Some(dynamic_memory) => {
            if let Some(dynamic_obj) = dynamic_memory.as_object_mut() {
                dynamic_obj.insert("enabled".to_string(), serde_json::Value::Bool(false));
            } else {
                *dynamic_memory = serde_json::json!({
                    "enabled": false
                });
            }
        }
        None => {
            obj.insert(
                "dynamicMemory".to_string(),
                serde_json::json!({
                    "enabled": false
                }),
            );
        }
    }
}

fn import_provider_credentials(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute("DELETE FROM provider_credentials", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            conn.execute(
                "INSERT INTO provider_credentials (id, provider_id, label, api_key_ref, api_key, base_url, default_model, headers, config)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    item.get("id").and_then(|v| v.as_str()),
                    item.get("provider_id").and_then(|v| v.as_str()),
                    item.get("label").and_then(|v| v.as_str()),
                    item.get("api_key_ref").and_then(|v| v.as_str()),
                    item.get("api_key").and_then(|v| v.as_str()),
                    item.get("base_url").and_then(|v| v.as_str()),
                    item.get("default_model").and_then(|v| v.as_str()),
                    item.get("headers").and_then(|v| v.as_str()),
                    item.get("config").and_then(|v| v.as_str()),
                ],
            ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    Ok(())
}

fn import_models(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute("DELETE FROM models", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        let normalize_scope_json_str = |raw: &str| -> String {
            let scope_order = ["text", "image", "audio"];
            let parsed: JsonValue =
                serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!(["text"]));
            let mut scopes: Vec<String> = vec![];
            if let Some(items) = parsed.as_array() {
                for v in items {
                    if let Some(s) = v.as_str() {
                        scopes.push(s.to_string());
                    }
                }
            }
            if !scopes.iter().any(|s| s.eq_ignore_ascii_case("text")) {
                scopes.push("text".to_string());
            }
            scopes.sort_by_key(|s| {
                scope_order
                    .iter()
                    .position(|o| o.eq_ignore_ascii_case(s))
                    .unwrap_or(scope_order.len())
            });
            scopes.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
            serde_json::to_string(&scopes).unwrap_or_else(|_| "[\"text\"]".into())
        };

        for item in arr {
            let legacy_model_type = item.get("model_type").and_then(|v| v.as_str());
            let input_scopes_raw = item
                .get("input_scopes")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    if legacy_model_type == Some("multimodel") {
                        Some("[\"text\",\"image\"]")
                    } else {
                        None
                    }
                })
                .unwrap_or("[\"text\"]");
            let output_scopes_raw = item
                .get("output_scopes")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    if legacy_model_type == Some("imagegeneration") {
                        Some("[\"text\",\"image\"]")
                    } else {
                        None
                    }
                })
                .unwrap_or("[\"text\"]");
            let input_scopes = normalize_scope_json_str(input_scopes_raw);
            let output_scopes = normalize_scope_json_str(output_scopes_raw);

            conn.execute(
                "INSERT INTO models (id, name, provider_id, provider_label, display_name, created_at,
                 provider_credential_id, model_type, input_scopes, output_scopes, advanced_model_settings, prompt_template_id, system_prompt)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    item.get("id").and_then(|v| v.as_str()),
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("provider_id").and_then(|v| v.as_str()),
                    item.get("provider_label").and_then(|v| v.as_str()),
                    item.get("display_name").and_then(|v| v.as_str()),
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("provider_credential_id").and_then(|v| v.as_str()),
                    item.get("model_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("chat"),
                    input_scopes,
                    output_scopes,
                    item.get("advanced_model_settings").and_then(|v| v.as_str()),
                    item.get("prompt_template_id").and_then(|v| v.as_str()),
                    item.get("system_prompt").and_then(|v| v.as_str()),
                ],
            ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    Ok(())
}

fn import_secrets(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute("DELETE FROM secrets", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            conn.execute(
                "INSERT INTO secrets (service, account, value, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    item.get("service").and_then(|v| v.as_str()),
                    item.get("account").and_then(|v| v.as_str()),
                    item.get("value").and_then(|v| v.as_str()),
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("updated_at").and_then(|v| v.as_i64()),
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    Ok(())
}

fn import_prompt_templates(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    // Delete all existing templates to ensure a clean state matching the backup
    conn.execute("DELETE FROM prompt_templates", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");

            let entries_str = if let Some(entries_value) = item.get("entries") {
                if entries_value.is_array() {
                    serde_json::to_string(entries_value).unwrap_or_else(|_| "[]".to_string())
                } else if let Some(s) = entries_value.as_str() {
                    s.to_string()
                } else {
                    "[]".to_string()
                }
            } else {
                "[]".to_string()
            };

            conn.execute(
                "INSERT OR REPLACE INTO prompt_templates (id, name, prompt_type, content, entries, condense_prompt_entries, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id,
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("prompt_type")
                        .or_else(|| item.get("promptType"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("undefined"),
                    item.get("content").and_then(|v| v.as_str()),
                    entries_str,
                    item.get("condense_prompt_entries")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("updated_at").and_then(|v| v.as_i64()),
                ],
            ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    Ok(())
}

fn import_personas(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute("DELETE FROM personas", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            conn.execute(
                "INSERT INTO personas (id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, is_default, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    item.get("id").and_then(|v| v.as_str()),
                    item.get("title").and_then(|v| v.as_str()),
                    item.get("description").and_then(|v| v.as_str()),
                    item.get("nickname").and_then(|v| v.as_str()),
                    item.get("avatar_path").and_then(|v| v.as_str()),
                    item.get("avatar_crop_x").and_then(|v| v.as_f64()),
                    item.get("avatar_crop_y").and_then(|v| v.as_f64()),
                    item.get("avatar_crop_scale").and_then(|v| v.as_f64()),
                    item.get("design_description").and_then(|v| v.as_str()),
                    item.get("design_reference_image_ids").and_then(|v| v.as_str()),
                    item.get("is_default").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("updated_at").and_then(|v| v.as_i64()),
                ],
            ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    Ok(())
}

fn import_characters(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;

    // Delete in correct order due to foreign keys
    conn.execute("DELETE FROM scene_variants", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM scenes", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM character_rules", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM characters", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut char_count = 0;
    let mut scene_count = 0;
    let mut variant_count = 0;
    let mut rule_count = 0;

    if let Some(arr) = data.as_array() {
        log_info(
            app,
            "backup",
            format!("Importing {} characters...", arr.len()),
        );

        for item in arr {
            let char_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let char_name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            // Insert character
            conn.execute(
                "INSERT INTO characters (id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, design_description, design_reference_image_ids, background_image_path, description, definition,
                 nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags,
                 default_scene_id, default_model_id, fallback_model_id, memory_type, prompt_template_id, group_chat_prompt_template_id, group_chat_roleplay_prompt_template_id, system_prompt,
                 voice_config, voice_autoplay, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors,
                 custom_text_color, custom_text_secondary, chat_appearance, default_chat_template_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37)",
                params![
                    char_id,
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("avatar_path").and_then(|v| v.as_str()),
                    item.get("avatar_crop_x").and_then(|v| v.as_f64()),
                    item.get("avatar_crop_y").and_then(|v| v.as_f64()),
                    item.get("avatar_crop_scale").and_then(|v| v.as_f64()),
                    item.get("design_description").and_then(|v| v.as_str()),
                    item.get("design_reference_image_ids").and_then(|v| v.as_str()),
                    item.get("background_image_path").and_then(|v| v.as_str()),
                    item.get("description").and_then(|v| v.as_str()),
                    item.get("definition")
                        .and_then(|v| v.as_str())
                        .or_else(|| item.get("description").and_then(|v| v.as_str())),
                    item.get("nickname").and_then(|v| v.as_str()),
                    item.get("scenario").and_then(|v| v.as_str()),
                    item.get("creator_notes").and_then(|v| v.as_str()),
                    item.get("creator").and_then(|v| v.as_str()),
                    item.get("creator_notes_multilingual")
                        .and_then(|v| v.as_str()),
                    item.get("source").and_then(|v| v.as_str()),
                    item.get("tags").and_then(|v| v.as_str()),
                    item.get("default_scene_id").and_then(|v| v.as_str()),
                    item.get("default_model_id").and_then(|v| v.as_str()),
                    item.get("fallback_model_id").and_then(|v| v.as_str()),
                    item.get("memory_type").and_then(|v| v.as_str()).unwrap_or("manual"),
                    item.get("prompt_template_id").and_then(|v| v.as_str()),
                    item.get("group_chat_prompt_template_id")
                        .and_then(|v| v.as_str()),
                    item.get("group_chat_roleplay_prompt_template_id")
                        .and_then(|v| v.as_str()),
                    item.get("system_prompt").and_then(|v| v.as_str()),
                    item.get("voice_config").and_then(|v| v.as_str()),
                    item.get("voice_autoplay")
                        .and_then(|v| v.as_i64())
                        .or_else(|| item.get("voice_autoplay").and_then(|v| v.as_bool()).map(|b| if b { 1 } else { 0 }))
                        .unwrap_or(0),
                    item.get("disable_avatar_gradient").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                    item.get("custom_gradient_enabled").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                    item.get("custom_gradient_colors").and_then(|v| v.as_str()),
                    item.get("custom_text_color").and_then(|v| v.as_str()),
                    item.get("custom_text_secondary").and_then(|v| v.as_str()),
                    item.get("chat_appearance").and_then(|v| v.as_str()),
                    item.get("default_chat_template_id")
                        .and_then(|v| v.as_str()),
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("updated_at").and_then(|v| v.as_i64()),
                ],
            ).map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert character '{}': {}", char_name, e)))?;
            char_count += 1;

            // Insert rules
            if let Some(rules) = item.get("rules").and_then(|v| v.as_array()) {
                for (idx, rule) in rules.iter().enumerate() {
                    if let Some(rule_str) = rule.as_str() {
                        conn.execute(
                            "INSERT INTO character_rules (character_id, idx, rule) VALUES (?1, ?2, ?3)",
                            params![char_id, idx as i64, rule_str],
                        )
                        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert rule for '{}': {}", char_name, e)))?;
                        rule_count += 1;
                    }
                }
            }

            // Insert scenes
            if let Some(scenes) = item.get("scenes").and_then(|v| v.as_array()) {
                for scene in scenes {
                    let scene_id = scene.get("id").and_then(|v| v.as_str()).unwrap_or("");

                    conn.execute(
                        "INSERT INTO scenes (id, character_id, content, direction, created_at, selected_variant_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            scene_id,
                            char_id,
                            scene.get("content").and_then(|v| v.as_str()),
                            scene.get("direction").and_then(|v| v.as_str()),
                            scene.get("created_at").and_then(|v| v.as_i64()),
                            scene.get("selected_variant_id").and_then(|v| v.as_str()),
                        ],
                    ).map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert scene for '{}': {}", char_name, e)))?;
                    scene_count += 1;

                    if let Some(variants) = scene.get("variants").and_then(|v| v.as_array()) {
                        for variant in variants {
                            conn.execute(
                                "INSERT INTO scene_variants (id, scene_id, content, direction, created_at)
                                 VALUES (?1, ?2, ?3, ?4, ?5)",
                                params![
                                    variant.get("id").and_then(|v| v.as_str()),
                                    scene_id,
                                    variant.get("content").and_then(|v| v.as_str()),
                                    variant.get("direction").and_then(|v| v.as_str()),
                                    variant.get("created_at").and_then(|v| v.as_i64()),
                                ],
                            )
                            .map_err(|e| {
                                format!("Failed to insert scene variant for '{}': {}", char_name, e)
                            })?;
                            variant_count += 1;
                        }
                    }
                }
            }
        }
    }

    log_info(
        app,
        "backup",
        format!(
            "Characters import complete: {} characters, {} scenes, {} variants, {} rules",
            char_count, scene_count, variant_count, rule_count
        ),
    );

    Ok(())
}

fn import_sessions(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;

    // Delete in correct order due to foreign keys
    conn.execute("DELETE FROM message_variants", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM messages", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM sessions", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut session_count = 0;
    let mut message_count = 0;
    let mut variant_count = 0;

    if let Some(arr) = data.as_array() {
        log_info(
            app,
            "backup",
            format!("Importing {} sessions...", arr.len()),
        );

        for item in arr {
            let session_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let character_id = item
                .get("character_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Insert session
            let voice_autoplay =
                item.get("voice_autoplay")
                    .and_then(|v| v.as_i64())
                    .or_else(|| {
                        item.get("voice_autoplay")
                            .and_then(|v| v.as_bool())
                            .map(|b| if b { 1 } else { 0 })
                    });

            conn.execute(
                "INSERT INTO sessions (id, character_id, title, background_image_path, system_prompt, selected_scene_id, persona_id, persona_disabled, voice_autoplay,
                 prompt_template_id, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k,
                 memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events,
                 memory_status, memory_error, memory_progress_step, archived, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)",
                params![
                    session_id,
                    character_id,
                    item.get("title").and_then(|v| v.as_str()),
                    item.get("background_image_path").and_then(|v| v.as_str()),
                    item.get("system_prompt").and_then(|v| v.as_str()),
                    item.get("selected_scene_id").and_then(|v| v.as_str()),
                    item.get("persona_id").and_then(|v| v.as_str()),
                    item.get("persona_disabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false) as i64,
                    voice_autoplay,
                    item.get("prompt_template_id").and_then(|v| v.as_str()),
                    item.get("temperature").and_then(|v| v.as_f64()),
                    item.get("top_p").and_then(|v| v.as_f64()),
                    item.get("max_output_tokens").and_then(|v| v.as_i64()),
                    item.get("frequency_penalty").and_then(|v| v.as_f64()),
                    item.get("presence_penalty").and_then(|v| v.as_f64()),
                    item.get("top_k").and_then(|v| v.as_i64()),
                    item.get("memories").and_then(|v| v.as_str()).unwrap_or("[]"),
                    item.get("memory_embeddings").and_then(|v| v.as_str()).unwrap_or("[]"),
                    item.get("memory_summary").and_then(|v| v.as_str()),
                    item.get("memory_summary_token_count").and_then(|v| v.as_i64()).unwrap_or(0),
                    item.get("memory_tool_events").and_then(|v| v.as_str()).unwrap_or("[]"),
                    item.get("memory_status").and_then(|v| v.as_str()),
                    item.get("memory_error").and_then(|v| v.as_str()),
                    item.get("memory_progress_step").and_then(|v| v.as_i64()),
                    item.get("archived").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("updated_at").and_then(|v| v.as_i64()),
                ],
            ).map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert session (character_id={}): {}", character_id, e)))?;
            session_count += 1;

            // Insert messages
            if let Some(messages) = item.get("messages").and_then(|v| v.as_array()) {
                for msg in messages {
                    let msg_id = msg.get("id").and_then(|v| v.as_str()).unwrap_or("");

                    conn.execute(
                        "INSERT INTO messages (id, session_id, role, content, created_at, prompt_tokens,
                         completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                        params![
                            msg_id,
                            session_id,
                            msg.get("role").and_then(|v| v.as_str()),
                            msg.get("content").and_then(|v| v.as_str()),
                            msg.get("created_at").and_then(|v| v.as_i64()),
                            msg.get("prompt_tokens").and_then(|v| v.as_i64()),
                            msg.get("completion_tokens").and_then(|v| v.as_i64()),
                            msg.get("total_tokens").and_then(|v| v.as_i64()),
                            msg.get("selected_variant_id").and_then(|v| v.as_str()),
                            msg.get("is_pinned").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                            msg.get("memory_refs").and_then(|v| v.as_str()).unwrap_or("[]"),
                            msg.get("used_lorebook_entries")
                                .and_then(|v| v.as_str())
                                .unwrap_or("[]"),
                            msg.get("attachments").and_then(|v| v.as_str()).unwrap_or("[]"),
                            msg.get("reasoning").and_then(|v| v.as_str()),
                        ],
                    ).map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert message in session {}: {}", session_id, e)))?;
                    message_count += 1;

                    // Insert message variants
                    if let Some(variants) = msg.get("variants").and_then(|v| v.as_array()) {
                        for variant in variants {
                            conn.execute(
                                "INSERT INTO message_variants (id, message_id, content, created_at,
                                 prompt_tokens, completion_tokens, total_tokens, reasoning)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                                params![
                                    variant.get("id").and_then(|v| v.as_str()),
                                    msg_id,
                                    variant.get("content").and_then(|v| v.as_str()),
                                    variant.get("created_at").and_then(|v| v.as_i64()),
                                    variant.get("prompt_tokens").and_then(|v| v.as_i64()),
                                    variant.get("completion_tokens").and_then(|v| v.as_i64()),
                                    variant.get("total_tokens").and_then(|v| v.as_i64()),
                                    variant.get("reasoning").and_then(|v| v.as_str()),
                                ],
                            )
                            .map_err(|e| {
                                crate::utils::err_msg(
                                    module_path!(),
                                    line!(),
                                    format!("Failed to insert message variant: {}", e),
                                )
                            })?;
                            variant_count += 1;
                        }
                    }
                }
            }
        }
    }

    log_info(
        app,
        "backup",
        format!(
            "Sessions import complete: {} sessions, {} messages, {} variants",
            session_count, message_count, variant_count
        ),
    );

    Ok(())
}

fn import_group_sessions(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;

    conn.execute("DELETE FROM group_message_variants", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM group_messages", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM group_participation", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM group_sessions", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut session_count = 0;
    let mut message_count = 0;
    let mut variant_count = 0;
    let mut participation_count = 0;

    if let Some(arr) = data.as_array() {
        log_info(
            app,
            "backup",
            format!("Importing {} group sessions...", arr.len()),
        );

        for item in arr {
            let session_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");

            conn.execute(
                "INSERT INTO group_sessions (id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived,
                 chat_type, starting_scene, background_image_path, lorebook_ids, disable_character_lorebooks,
                 memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, memory_progress_step, speaker_selection_method, memory_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
                params![
                    session_id,
                    item.get("group_character_id").and_then(|v| v.as_str()),
                    item.get("name").and_then(|v| v.as_str()).unwrap_or("Group Chat"),
                    item.get("character_ids").and_then(|v| v.as_str()).unwrap_or("[]"),
                    item.get("muted_character_ids")
                        .and_then(|v| v.as_str())
                        .unwrap_or("[]"),
                    item.get("persona_id").and_then(|v| v.as_str()),
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("updated_at").and_then(|v| v.as_i64()),
                    item.get("archived").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                    item.get("chat_type").and_then(|v| v.as_str()).unwrap_or("conversation"),
                    item.get("starting_scene").and_then(|v| v.as_str()),
                    item.get("background_image_path").and_then(|v| v.as_str()),
                    item.get("lorebook_ids").and_then(|v| v.as_str()).unwrap_or("[]"),
                    item.get("disable_character_lorebooks")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false) as i64,
                    item.get("memories").and_then(|v| v.as_str()).unwrap_or("[]"),
                    item.get("memory_embeddings").and_then(|v| v.as_str()).unwrap_or("[]"),
                    item.get("memory_summary").and_then(|v| v.as_str()).unwrap_or(""),
                    item.get("memory_summary_token_count").and_then(|v| v.as_i64()).unwrap_or(0),
                    item.get("memory_tool_events").and_then(|v| v.as_str()).unwrap_or("[]"),
                    item.get("memory_status").and_then(|v| v.as_str()),
                    item.get("memory_error").and_then(|v| v.as_str()),
                    item.get("memory_progress_step").and_then(|v| v.as_i64()),
                    item.get("speaker_selection_method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("llm"),
                    item.get("memory_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("manual"),
                ],
            )
            .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert group session {}: {}", session_id, e)))?;
            session_count += 1;

            if let Some(participants) = item.get("participation").and_then(|v| v.as_array()) {
                for part in participants {
                    conn.execute(
                        "INSERT INTO group_participation (id, session_id, character_id, speak_count, last_spoke_turn, last_spoke_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            part.get("id").and_then(|v| v.as_str()),
                            session_id,
                            part.get("character_id").and_then(|v| v.as_str()),
                            part.get("speak_count").and_then(|v| v.as_i64()).unwrap_or(0),
                            part.get("last_spoke_turn").and_then(|v| v.as_i64()),
                            part.get("last_spoke_at").and_then(|v| v.as_i64()),
                        ],
                    )
                    .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert group participation in session {}: {}", session_id, e)))?;
                    participation_count += 1;
                }
            }

            if let Some(messages) = item.get("messages").and_then(|v| v.as_array()) {
                for msg in messages {
                    let msg_id = msg.get("id").and_then(|v| v.as_str()).unwrap_or("");

                    conn.execute(
                        "INSERT INTO group_messages (id, session_id, role, content, speaker_character_id, turn_number, created_at,
                         prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, attachments, used_lorebook_entries, reasoning, selection_reasoning, model_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                        params![
                            msg_id,
                            session_id,
                            msg.get("role").and_then(|v| v.as_str()),
                            msg.get("content").and_then(|v| v.as_str()),
                            msg.get("speaker_character_id").and_then(|v| v.as_str()),
                            msg.get("turn_number").and_then(|v| v.as_i64()).unwrap_or(0),
                            msg.get("created_at").and_then(|v| v.as_i64()),
                            msg.get("prompt_tokens").and_then(|v| v.as_i64()),
                            msg.get("completion_tokens").and_then(|v| v.as_i64()),
                            msg.get("total_tokens").and_then(|v| v.as_i64()),
                            msg.get("selected_variant_id").and_then(|v| v.as_str()),
                            msg.get("is_pinned").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                            msg.get("attachments").and_then(|v| v.as_str()).unwrap_or("[]"),
                            msg.get("used_lorebook_entries").and_then(|v| v.as_str()).unwrap_or("[]"),
                            msg.get("reasoning").and_then(|v| v.as_str()),
                            msg.get("selection_reasoning").and_then(|v| v.as_str()),
                            msg.get("model_id").and_then(|v| v.as_str()),
                        ],
                    )
                    .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert group message in session {}: {}", session_id, e)))?;
                    message_count += 1;

                    if let Some(variants) = msg.get("variants").and_then(|v| v.as_array()) {
                        for variant in variants {
                            conn.execute(
                                "INSERT INTO group_message_variants (id, message_id, content, speaker_character_id, created_at,
                                 prompt_tokens, completion_tokens, total_tokens, reasoning, selection_reasoning, model_id)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                                params![
                                    variant.get("id").and_then(|v| v.as_str()),
                                    msg_id,
                                    variant.get("content").and_then(|v| v.as_str()),
                                    variant.get("speaker_character_id").and_then(|v| v.as_str()),
                                    variant.get("created_at").and_then(|v| v.as_i64()),
                                    variant.get("prompt_tokens").and_then(|v| v.as_i64()),
                                    variant.get("completion_tokens").and_then(|v| v.as_i64()),
                                    variant.get("total_tokens").and_then(|v| v.as_i64()),
                                    variant.get("reasoning").and_then(|v| v.as_str()),
                                    variant.get("selection_reasoning").and_then(|v| v.as_str()),
                                    variant.get("model_id").and_then(|v| v.as_str()),
                                ],
                            )
                            .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert group message variant: {}", e)))?;
                            variant_count += 1;
                        }
                    }
                }
            }
        }
    }

    log_info(
        app,
        "backup",
        format!(
            "Group sessions import complete: {} sessions, {} participants, {} messages, {} variants",
            session_count, participation_count, message_count, variant_count
        ),
    );

    Ok(())
}

fn import_usage_records(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;

    // Delete in correct order due to foreign keys
    conn.execute("DELETE FROM usage_metadata", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM usage_records", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            let record_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");

            // Insert usage record
            conn.execute(
                "INSERT INTO usage_records (id, timestamp, session_id, character_id, character_name,
                 model_id, model_name, provider_id, provider_label, operation_type, finish_reason, prompt_tokens,
                 completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens,
                 prompt_cost, completion_cost, total_cost, success, error_message)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
                params![
                    record_id,
                    item.get("timestamp").and_then(|v| v.as_i64()),
                    item.get("session_id").and_then(|v| v.as_str()),
                    item.get("character_id").and_then(|v| v.as_str()),
                    item.get("character_name").and_then(|v| v.as_str()),
                    item.get("model_id").and_then(|v| v.as_str()),
                    item.get("model_name").and_then(|v| v.as_str()),
                    item.get("provider_id").and_then(|v| v.as_str()),
                    item.get("provider_label").and_then(|v| v.as_str()),
                    item.get("operation_type").and_then(|v| v.as_str()),
                    item.get("finish_reason").and_then(|v| v.as_str()),
                    item.get("prompt_tokens").and_then(|v| v.as_i64()),
                    item.get("completion_tokens").and_then(|v| v.as_i64()),
                    item.get("total_tokens").and_then(|v| v.as_i64()),
                    item.get("memory_tokens").and_then(|v| v.as_i64()),
                    item.get("summary_tokens").and_then(|v| v.as_i64()),
                    item.get("reasoning_tokens").and_then(|v| v.as_i64()),
                    item.get("image_tokens").and_then(|v| v.as_i64()),
                    item.get("prompt_cost").and_then(|v| v.as_f64()),
                    item.get("completion_cost").and_then(|v| v.as_f64()),
                    item.get("total_cost").and_then(|v| v.as_f64()),
                    item.get("success").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                    item.get("error_message").and_then(|v| v.as_str()),
                ],
            ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            // Insert metadata
            if let Some(metadata) = item.get("metadata").and_then(|v| v.as_array()) {
                for meta in metadata {
                    conn.execute(
                        "INSERT INTO usage_metadata (usage_id, key, value) VALUES (?1, ?2, ?3)",
                        params![
                            record_id,
                            meta.get("key").and_then(|v| v.as_str()),
                            meta.get("value").and_then(|v| v.as_str()),
                        ],
                    )
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                }
            }
        }
    }
    Ok(())
}

fn import_lorebooks(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;

    // Delete existing lorebook entries and lorebooks (entries have FK to lorebooks)
    conn.execute("DELETE FROM lorebook_entries", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM lorebooks", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            let lorebook_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");

            conn.execute(
                "INSERT INTO lorebooks (id, name, avatar_path, keyword_detection_mode, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    lorebook_id,
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("avatar_path").and_then(|v| v.as_str()),
                    item.get("keyword_detection_mode")
                        .and_then(|v| v.as_str())
                        .unwrap_or("recent_message_window"),
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("updated_at").and_then(|v| v.as_i64()),
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            // Insert entries (table schema has no title column)
            if let Some(entries) = item.get("entries").and_then(|v| v.as_array()) {
                for entry in entries {
                    conn.execute(
                        "INSERT INTO lorebook_entries (id, lorebook_id, title, enabled, always_active, keywords, case_sensitive, content, priority, display_order, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                        params![
                            entry.get("id").and_then(|v| v.as_str()),
                            lorebook_id,
                            entry.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                            entry.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) as i64,
                            entry.get("always_active").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                            entry.get("keywords").and_then(|v| v.as_str()).unwrap_or("[]"),
                            entry.get("case_sensitive")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false) as i64,
                            entry.get("content").and_then(|v| v.as_str()),
                            entry.get("priority").and_then(|v| v.as_i64()).unwrap_or(0),
                            entry.get("display_order").and_then(|v| v.as_i64()).unwrap_or(0),
                            entry.get("created_at").and_then(|v| v.as_i64()),
                            entry.get("updated_at").and_then(|v| v.as_i64()),
                        ],
                    ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                }
            }
        }
    }
    Ok(())
}

fn import_character_lorebooks(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;

    // Delete existing character-lorebook links
    conn.execute("DELETE FROM character_lorebooks", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            conn.execute(
                "INSERT INTO character_lorebooks (character_id, lorebook_id, enabled, display_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    item.get("character_id").and_then(|v| v.as_str()),
                    item.get("lorebook_id").and_then(|v| v.as_str()),
                    item.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) as i64,
                    item.get("display_order").and_then(|v| v.as_i64()).unwrap_or(0),
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("updated_at").and_then(|v| v.as_i64()),
                ],
            ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    Ok(())
}

fn import_chat_templates(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute("DELETE FROM chat_template_messages", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute("DELETE FROM chat_templates", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            let template_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            conn.execute(
                "INSERT INTO chat_templates (id, character_id, name, scene_id, prompt_template_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    template_id,
                    item.get("character_id").and_then(|v| v.as_str()),
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("scene_id").and_then(|v| v.as_str()),
                    item.get("prompt_template_id").and_then(|v| v.as_str()),
                    item.get("created_at").and_then(|v| v.as_i64()),
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            if let Some(messages) = item.get("messages").and_then(|v| v.as_array()) {
                for (fallback_idx, message) in messages.iter().enumerate() {
                    conn.execute(
                        "INSERT INTO chat_template_messages (id, template_id, idx, role, content)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            message.get("id").and_then(|v| v.as_str()),
                            template_id,
                            message
                                .get("idx")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(fallback_idx as i64),
                            message.get("role").and_then(|v| v.as_str()),
                            message.get("content").and_then(|v| v.as_str()),
                        ],
                    )
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                }
            }
        }
    }

    Ok(())
}

fn import_creation_helper_sessions(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute("DELETE FROM creation_helper_sessions", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            conn.execute(
                "INSERT INTO creation_helper_sessions (id, creation_goal, status, session_json, uploaded_images_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    item.get("id").and_then(|v| v.as_str()),
                    item.get("creation_goal").and_then(|v| v.as_str()),
                    item.get("status").and_then(|v| v.as_str()),
                    item.get("session_json").and_then(|v| v.as_str()),
                    item.get("uploaded_images_json").and_then(|v| v.as_str()),
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("updated_at").and_then(|v| v.as_i64()),
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }

    Ok(())
}

fn import_group_characters(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute("DELETE FROM group_characters", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            conn.execute(
                "INSERT INTO group_characters (id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived,
                 chat_type, starting_scene, background_image_path, lorebook_ids, disable_character_lorebooks, speaker_selection_method, memory_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    item.get("id").and_then(|v| v.as_str()),
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("character_ids").and_then(|v| v.as_str()).unwrap_or("[]"),
                    item.get("muted_character_ids")
                        .and_then(|v| v.as_str())
                        .unwrap_or("[]"),
                    item.get("persona_id").and_then(|v| v.as_str()),
                    item.get("created_at").and_then(|v| v.as_i64()),
                    item.get("updated_at").and_then(|v| v.as_i64()),
                    item.get("archived").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                    item.get("chat_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("conversation"),
                    item.get("starting_scene").and_then(|v| v.as_str()),
                    item.get("background_image_path").and_then(|v| v.as_str()),
                    item.get("lorebook_ids").and_then(|v| v.as_str()).unwrap_or("[]"),
                    item.get("disable_character_lorebooks")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false) as i64,
                    item.get("speaker_selection_method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("llm"),
                    item.get("memory_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("manual"),
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }

    Ok(())
}

fn import_model_pricing_cache(app: &tauri::AppHandle, data: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute("DELETE FROM model_pricing_cache", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(arr) = data.as_array() {
        for item in arr {
            conn.execute(
                "INSERT INTO model_pricing_cache (model_id, pricing_json, cached_at)
                 VALUES (?1, ?2, ?3)",
                params![
                    item.get("model_id").and_then(|v| v.as_str()),
                    item.get("pricing_json").and_then(|v| v.as_str()),
                    item.get("cached_at").and_then(|v| v.as_i64()),
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }

    Ok(())
}

/// Check if a backup file requires a password
#[tauri::command]
pub fn backup_check_encrypted(app: tauri::AppHandle, backup_path: String) -> Result<bool, String> {
    let file = open_backup_file(&app, &backup_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read backup archive: {}", e),
        )
    })?;

    // Read manifest
    let mut manifest_file = archive.by_name("manifest.json").map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid backup: missing manifest: {}", e),
        )
    })?;

    let mut manifest_str = String::new();
    manifest_file
        .read_to_string(&mut manifest_str)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let manifest: BackupManifest = serde_json::from_str(&manifest_str).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Invalid manifest: {}", e))
    })?;

    Ok(manifest.encrypted)
}

/// Verify password for an encrypted backup
#[tauri::command]
pub fn backup_verify_password(
    app: tauri::AppHandle,
    backup_path: String,
    password: String,
) -> Result<bool, String> {
    let file = open_backup_file(&app, &backup_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read backup archive: {}", e),
        )
    })?;

    // Read manifest
    let mut manifest_file = archive.by_name("manifest.json").map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid backup: missing manifest: {}", e),
        )
    })?;

    let mut manifest_str = String::new();
    manifest_file
        .read_to_string(&mut manifest_str)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let manifest: BackupManifest = serde_json::from_str(&manifest_str).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Invalid manifest: {}", e))
    })?;

    require_encrypted_backup(&manifest)?;

    let salt_b64 = manifest
        .salt
        .ok_or_else(|| "Missing salt in encrypted backup".to_string())?;
    let nonce_b64 = manifest
        .nonce
        .ok_or_else(|| "Missing nonce in encrypted backup".to_string())?;

    let salt_vec = general_purpose::STANDARD
        .decode(&salt_b64)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let nonce_vec = general_purpose::STANDARD
        .decode(&nonce_b64)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    salt.copy_from_slice(&salt_vec);
    nonce.copy_from_slice(&nonce_vec);

    let password =
        require_non_empty_password(Some(password.as_str()), "backup password verification")?;
    let key = derive_key_from_password(password, &salt);

    // Try to decrypt the marker
    drop(manifest_file);
    let mut marker_file = archive.by_name("encrypted_marker.bin").map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid backup: missing marker: {}", e),
        )
    })?;

    let mut encrypted_marker = Vec::new();
    marker_file
        .read_to_end(&mut encrypted_marker)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    match decrypt_data(&encrypted_marker, &key, &nonce) {
        Ok(decrypted) => Ok(decrypted == b"LETTUCE_BACKUP_VERIFIED"),
        Err(_) => Ok(false),
    }
}

/// Get backup info without importing
#[tauri::command]
pub fn backup_get_info(
    app: tauri::AppHandle,
    backup_path: String,
) -> Result<serde_json::Value, String> {
    let file = open_backup_file(&app, &backup_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read backup archive: {}", e),
        )
    })?;

    // Read manifest
    let mut manifest_file = archive.by_name("manifest.json").map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid backup: missing manifest: {}", e),
        )
    })?;

    let mut manifest_str = String::new();
    manifest_file
        .read_to_string(&mut manifest_str)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let manifest: BackupManifest = serde_json::from_str(&manifest_str).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Invalid manifest: {}", e))
    })?;

    // Count files
    drop(manifest_file);
    let total_files = archive.len();
    let mut image_count = 0;
    let mut avatar_count = 0;
    let mut attachment_count = 0;
    let mut session_attachment_count = 0;
    let mut generated_image_count = 0;

    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name();
            if name.starts_with("images/") {
                image_count += 1;
            } else if name.starts_with("avatars/") {
                avatar_count += 1;
            } else if name.starts_with("attachments/") {
                attachment_count += 1;
            } else if name.starts_with("sessions/") {
                session_attachment_count += 1;
            } else if name.starts_with("generated_images/") {
                generated_image_count += 1;
            }
        }
    }

    Ok(serde_json::json!({
        "version": manifest.version,
        "createdAt": manifest.created_at,
        "appVersion": manifest.app_version,
        "encrypted": manifest.encrypted,
        "totalFiles": total_files,
        "imageCount": image_count,
        "avatarCount": avatar_count,
        "attachmentCount": attachment_count,
        "sessionAttachmentCount": session_attachment_count,
        "generatedImageCount": generated_image_count,
    }))
}

/// Import a backup file, replacing all existing data (v2 format - JSON-based)
#[tauri::command]
pub async fn backup_import(
    app: tauri::AppHandle,
    backup_path: String,
    password: Option<String>,
) -> Result<(), String> {
    let storage = storage_root(&app)?;

    // First, read and validate manifest
    let manifest: BackupManifest = {
        let file = open_backup_file(&app, &backup_path)?;
        let mut archive = ZipArchive::new(file).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read backup archive: {}", e),
            )
        })?;

        let mut manifest_file = archive.by_name("manifest.json").map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid backup: missing manifest: {}", e),
            )
        })?;

        let mut manifest_str = String::new();
        manifest_file
            .read_to_string(&mut manifest_str)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        serde_json::from_str(&manifest_str).map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Invalid manifest: {}", e))
        })?
    };

    log_info(
        &app,
        "backup",
        format!("Starting backup import v2 from {:?}", backup_path),
    );

    require_encrypted_backup(&manifest)?;

    // Check backup version - only support v2
    if manifest.version < BACKUP_VERSION {
        return Err(format!(
            "Backup version {} is not supported. This app requires backup version {}.",
            manifest.version, BACKUP_VERSION
        ));
    }

    // Prepare encryption params if encrypted
    let encryption_params: Option<([u8; 32], [u8; 24])> = if manifest.encrypted {
        let pwd = require_non_empty_password(password.as_deref(), "backup import")?;

        let salt_b64 = manifest
            .salt
            .as_ref()
            .ok_or_else(|| "Missing salt".to_string())?;
        let nonce_b64 = manifest
            .nonce
            .as_ref()
            .ok_or_else(|| "Missing nonce".to_string())?;

        let salt_vec = general_purpose::STANDARD
            .decode(salt_b64)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let nonce_vec = general_purpose::STANDARD
            .decode(nonce_b64)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut salt = [0u8; 16];
        let mut nonce = [0u8; 24];
        salt.copy_from_slice(&salt_vec);
        nonce.copy_from_slice(&nonce_vec);

        let key = derive_key_from_password(pwd, &salt);

        // Verify marker BEFORE proceeding - this validates the password
        let file = open_backup_file(&app, &backup_path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut marker_file = archive.by_name("encrypted_marker.bin").map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid backup: missing encryption marker: {}", e),
            )
        })?;

        let mut encrypted_marker = Vec::new();
        marker_file
            .read_to_end(&mut encrypted_marker)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let decrypted = decrypt_data(&encrypted_marker, &key, &nonce)
            .map_err(|_| "Invalid password - decryption failed".to_string())?;

        if decrypted != b"LETTUCE_BACKUP_VERIFIED" {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Invalid password - verification marker mismatch",
            ));
        }

        log_info(&app, "backup", "Password verified successfully");
        Some((key, nonce))
    } else {
        None
    };

    // Helper to read and optionally decrypt a file from the archive
    let read_backup_file = |archive: &mut ZipArchive<File>,
                            path: &str,
                            enc_params: &Option<([u8; 32], [u8; 24])>|
     -> Result<Option<Vec<u8>>, String> {
        // Try encrypted version first if we have encryption params
        let encrypted_path = format!("{}.enc", path);

        if let Some((ref key, ref nonce)) = enc_params {
            if let Ok(mut file) = archive.by_name(&encrypted_path) {
                let mut contents = Vec::new();
                file.read_to_end(&mut contents)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                let decrypted = decrypt_data(&contents, key, nonce).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to decrypt {}: {}", path, e),
                    )
                })?;
                return Ok(Some(decrypted));
            }
        }

        // Try unencrypted version
        if let Ok(mut file) = archive.by_name(path) {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            return Ok(Some(contents));
        }

        Ok(None)
    };

    // Re-open archive for reading data files
    let file = open_backup_file(&app, &backup_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    log_info(&app, "backup", "Reading JSON data files...");

    // Read all JSON data files
    let meta_data = read_backup_file(&mut archive, "data/meta.json", &encryption_params)?;
    let settings_data = read_backup_file(&mut archive, "data/settings.json", &encryption_params)?;
    let provider_credentials_data = read_backup_file(
        &mut archive,
        "data/provider_credentials.json",
        &encryption_params,
    )?;
    let models_data = read_backup_file(&mut archive, "data/models.json", &encryption_params)?;
    let model_pricing_cache_data = read_backup_file(
        &mut archive,
        "data/model_pricing_cache.json",
        &encryption_params,
    )?;
    let secrets_data = read_backup_file(&mut archive, "data/secrets.json", &encryption_params)?;
    let prompt_templates_data = read_backup_file(
        &mut archive,
        "data/prompt_templates.json",
        &encryption_params,
    )?;
    let chat_templates_data =
        read_backup_file(&mut archive, "data/chat_templates.json", &encryption_params)?;
    let personas_data = read_backup_file(&mut archive, "data/personas.json", &encryption_params)?;
    let characters_data =
        read_backup_file(&mut archive, "data/characters.json", &encryption_params)?;
    let sessions_data = read_backup_file(&mut archive, "data/sessions.json", &encryption_params)?;
    let creation_helper_sessions_data = read_backup_file(
        &mut archive,
        "data/creation_helper_sessions.json",
        &encryption_params,
    )?;
    let group_characters_data = read_backup_file(
        &mut archive,
        "data/group_characters.json",
        &encryption_params,
    )?;
    let group_sessions_data =
        read_backup_file(&mut archive, "data/group_sessions.json", &encryption_params)?;
    let usage_records_data =
        read_backup_file(&mut archive, "data/usage_records.json", &encryption_params)?;
    let lorebooks_data = read_backup_file(&mut archive, "data/lorebooks.json", &encryption_params)?;
    let character_lorebooks_data = read_backup_file(
        &mut archive,
        "data/character_lorebooks.json",
        &encryption_params,
    )?;

    log_info(&app, "backup", "Importing data to database...");

    // Import data in correct order (respecting foreign key constraints)
    if let Some(data) = meta_data {
        log_info(&app, "backup", "Found meta data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse meta JSON: {}", e),
            )
        })?;
        import_meta(&app, &json_value)?;
        log_info(&app, "backup", "Meta imported");
    } else {
        log_info(&app, "backup", "No meta data found");
    }

    // Settings first (no dependencies)
    if let Some(data) = settings_data {
        log_info(&app, "backup", "Found settings data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse settings JSON: {}", e),
            )
        })?;
        import_settings(&app, &json_value)?;
        log_info(&app, "backup", "Settings imported");
    } else {
        log_info(&app, "backup", "No settings data found");
    }

    // Provider credentials (no dependencies)
    if let Some(data) = provider_credentials_data {
        log_info(&app, "backup", "Found provider_credentials data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse provider_credentials JSON: {}", e),
            )
        })?;
        import_provider_credentials(&app, &json_value)?;
        log_info(&app, "backup", "Provider credentials imported");
    } else {
        log_info(&app, "backup", "No provider_credentials data found");
    }

    // Models (depends on provider_credentials)
    if let Some(data) = models_data {
        log_info(&app, "backup", "Found models data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse models JSON: {}", e),
            )
        })?;
        import_models(&app, &json_value)?;
        log_info(&app, "backup", "Models imported");
    } else {
        log_info(&app, "backup", "No models data found");
    }

    if let Some(data) = model_pricing_cache_data {
        log_info(&app, "backup", "Found model_pricing_cache data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse model_pricing_cache JSON: {}", e),
            )
        })?;
        import_model_pricing_cache(&app, &json_value)?;
        log_info(&app, "backup", "Model pricing cache imported");
    } else {
        log_info(&app, "backup", "No model_pricing_cache data found");
    }

    // Secrets (no dependencies)
    if let Some(data) = secrets_data {
        log_info(&app, "backup", "Found secrets data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse secrets JSON: {}", e),
            )
        })?;
        import_secrets(&app, &json_value)?;
        log_info(&app, "backup", "Secrets imported");
    } else {
        log_info(&app, "backup", "No secrets data found");
    }

    // Prompt templates (no dependencies)
    if let Some(data) = prompt_templates_data {
        log_info(&app, "backup", "Found prompt_templates data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse prompt_templates JSON: {}", e),
            )
        })?;
        import_prompt_templates(&app, &json_value)?;
        log_info(&app, "backup", "Prompt templates imported");
    } else {
        log_info(&app, "backup", "No prompt_templates data found");
    }

    // Personas (no dependencies)
    if let Some(data) = personas_data {
        log_info(&app, "backup", "Found personas data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse personas JSON: {}", e),
            )
        })?;
        import_personas(&app, &json_value)?;
        log_info(&app, "backup", "Personas imported");
    } else {
        log_info(&app, "backup", "No personas data found");
    }

    // Characters (no dependencies)
    if let Some(data) = characters_data {
        log_info(&app, "backup", "Found characters data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse characters JSON: {}", e),
            )
        })?;
        import_characters(&app, &json_value)?;
        log_info(&app, "backup", "Characters imported");
    } else {
        log_info(&app, "backup", "No characters data found");
    }

    if let Some(data) = chat_templates_data {
        log_info(&app, "backup", "Found chat_templates data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse chat_templates JSON: {}", e),
            )
        })?;
        import_chat_templates(&app, &json_value)?;
        log_info(&app, "backup", "Chat templates imported");
    } else {
        log_info(&app, "backup", "No chat_templates data found");
    }

    // Sessions (depends on personas and characters)
    if let Some(data) = sessions_data {
        log_info(&app, "backup", "Found sessions data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse sessions JSON: {}", e),
            )
        })?;
        import_sessions(&app, &json_value)?;
        log_info(&app, "backup", "Sessions imported");
    } else {
        log_info(&app, "backup", "No sessions data found");
    }

    if let Some(data) = creation_helper_sessions_data {
        log_info(&app, "backup", "Found creation_helper_sessions data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse creation_helper_sessions JSON: {}", e),
            )
        })?;
        import_creation_helper_sessions(&app, &json_value)?;
        log_info(&app, "backup", "Creation helper sessions imported");
    } else {
        log_info(&app, "backup", "No creation_helper_sessions data found");
    }

    if let Some(data) = group_characters_data {
        log_info(&app, "backup", "Found group_characters data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse group_characters JSON: {}", e),
            )
        })?;
        import_group_characters(&app, &json_value)?;
        log_info(&app, "backup", "Group character presets imported");
    } else {
        log_info(&app, "backup", "No group_characters data found");
    }

    // Group sessions (depends on personas and characters)
    if let Some(data) = group_sessions_data {
        log_info(&app, "backup", "Found group sessions data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse group sessions JSON: {}", e),
            )
        })?;
        import_group_sessions(&app, &json_value)?;
        log_info(&app, "backup", "Group sessions imported");
    } else {
        log_info(&app, "backup", "No group sessions data found");
    }

    // Usage records (depends on sessions)
    if let Some(data) = usage_records_data {
        log_info(&app, "backup", "Found usage_records data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse usage_records JSON: {}", e),
            )
        })?;
        import_usage_records(&app, &json_value)?;
        log_info(&app, "backup", "Usage records imported");
    } else {
        log_info(&app, "backup", "No usage_records data found");
    }

    // Lorebooks (no dependencies, import before character_lorebooks)
    if let Some(data) = lorebooks_data {
        log_info(&app, "backup", "Found lorebooks data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse lorebooks JSON: {}", e),
            )
        })?;
        import_lorebooks(&app, &json_value)?;
        log_info(&app, "backup", "Lorebooks imported");
    } else {
        log_info(&app, "backup", "No lorebooks data found");
    }

    // Character-lorebook links (depends on characters and lorebooks)
    if let Some(data) = character_lorebooks_data {
        log_info(&app, "backup", "Found character_lorebooks data");
        let json_str = String::from_utf8(data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse character_lorebooks JSON: {}", e),
            )
        })?;
        import_character_lorebooks(&app, &json_value)?;
        log_info(&app, "backup", "Character-lorebook links imported");
    } else {
        log_info(&app, "backup", "No character_lorebooks data found");
    }

    log_info(&app, "backup", "Extracting media files...");

    // Extract media files to staging directory, then copy
    let staging_dir = storage.join(".import_staging");
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    fs::create_dir_all(&staging_dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Re-open archive for media extraction
    let file = open_backup_file(&app, &backup_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Extract media files (images, avatars, attachments)
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let file_name = file.name().to_string();
        if !archive_name_targets_media(&file_name) {
            continue;
        }

        let relative_path = sanitize_media_archive_name(&file_name, false)?;

        if file.is_dir() {
            let outpath = staging_dir.join(&relative_path);
            fs::create_dir_all(&outpath)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        } else {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            // Decrypt if needed
            let (outpath, final_contents) = if let Some((ref key, ref nonce)) = encryption_params {
                if file_name.ends_with(".enc") {
                    let decrypted = decrypt_data(&contents, key, nonce).map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Failed to decrypt {}: {}", file_name, e),
                        )
                    })?;
                    let out_name = sanitize_media_archive_name(&file_name, true)?;
                    (staging_dir.join(out_name), decrypted)
                } else {
                    (staging_dir.join(&relative_path), contents)
                }
            } else {
                (staging_dir.join(&relative_path), contents)
            };

            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }

            let mut outfile = File::create(&outpath)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            outfile
                .write_all(&final_contents)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }

    // Copy media from staging to actual locations
    let images_dir = storage.join("images");
    let avatars_dir = storage.join("avatars");
    let attachments_dir = storage.join("attachments");
    let sessions_dir = storage.join("sessions");
    let generated_images_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .join("generated_images");

    // Clear existing media directories
    if images_dir.exists() {
        fs::remove_dir_all(&images_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if avatars_dir.exists() {
        fs::remove_dir_all(&avatars_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if attachments_dir.exists() {
        fs::remove_dir_all(&attachments_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    let staged_images = staging_dir.join("images");
    if staged_images.exists() {
        copy_dir_all(&staged_images, &images_dir)?;
        log_info(&app, "backup", "Images restored");
    }

    let staged_avatars = staging_dir.join("avatars");
    if staged_avatars.exists() {
        copy_dir_all(&staged_avatars, &avatars_dir)?;
        log_info(&app, "backup", "Avatars restored");
    }

    let staged_attachments = staging_dir.join("attachments");
    if staged_attachments.exists() {
        copy_dir_all(&staged_attachments, &attachments_dir)?;
        log_info(&app, "backup", "Attachments restored");
    }

    let staged_sessions = staging_dir.join("sessions");
    if staged_sessions.exists() {
        if sessions_dir.exists() {
            fs::remove_dir_all(&sessions_dir)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
        copy_dir_all(&staged_sessions, &sessions_dir)?;
        log_info(&app, "backup", "Session attachments restored");
    }

    let staged_generated_images = staging_dir.join("generated_images");
    if staged_generated_images.exists() {
        if generated_images_dir.exists() {
            fs::remove_dir_all(&generated_images_dir)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
        copy_dir_all(&staged_generated_images, &generated_images_dir)?;
        log_info(&app, "backup", "Generated images restored");
    }

    fs::remove_dir_all(&staging_dir).ok();

    // Cleanup temporary file if it was created by the frontend (Android content URI)
    if backup_path.ends_with("backup_import_temp.lettuce") {
        if let Err(e) = fs::remove_file(&backup_path) {
            log_info(
                &app,
                "backup",
                &format!("Failed to delete temp backup file: {}", e),
            );
        } else {
            log_info(&app, "backup", "Deleted temp backup file");
        }
    }

    log_info(&app, "backup", "Backup import v2 complete!");

    app.emit("database-reloaded", ()).ok();

    Ok(())
}

/// Helper to copy directory recursively
fn copy_dir_all(src: &PathBuf, dst: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let relative = path
            .strip_prefix(src)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let target = dst.join(relative);

        if path.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }
            fs::copy(path, &target)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{disable_dynamic_memory_in_advanced_settings, sanitize_media_archive_name};
    use std::path::PathBuf;

    #[test]
    fn sanitize_media_archive_name_accepts_safe_paths() {
        assert_eq!(
            sanitize_media_archive_name("images/example.png", false).unwrap(),
            PathBuf::from("images/example.png")
        );
        assert_eq!(
            sanitize_media_archive_name("images/example.png.enc", true).unwrap(),
            PathBuf::from("images/example.png")
        );
        assert_eq!(
            sanitize_media_archive_name("avatars/character-1/", false).unwrap(),
            PathBuf::from("avatars/character-1")
        );
    }

    #[test]
    fn sanitize_media_archive_name_rejects_traversal() {
        assert!(sanitize_media_archive_name("images/../../tmp/pwned", false).is_err());
        assert!(sanitize_media_archive_name("images\\..\\..\\tmp\\pwned", false).is_err());
        assert!(sanitize_media_archive_name("/tmp/pwned", false).is_err());
    }

    #[test]
    fn require_encrypted_backup_rejects_plaintext_archives() {
        let manifest = super::BackupManifest {
            version: super::BACKUP_VERSION,
            created_at: 0,
            app_version: "test".to_string(),
            encrypted: false,
            salt: None,
            nonce: None,
        };

        assert!(super::require_encrypted_backup(&manifest).is_err());
    }

    #[test]
    fn disable_dynamic_memory_preserves_imported_settings() {
        let mut settings = serde_json::json!({
            "dynamicMemory": {
                "enabled": true,
                "summaryMessageInterval": 42,
                "maxEntries": 99,
                "retrievalLimit": 7,
                "contextEnrichmentEnabled": false
            },
            "groupDynamicMemory": {
                "enabled": true,
                "summaryMessageInterval": 11
            }
        });

        disable_dynamic_memory_in_advanced_settings(&mut settings);

        assert_eq!(
            settings["dynamicMemory"]["enabled"],
            serde_json::Value::Bool(false)
        );
        assert_eq!(settings["dynamicMemory"]["summaryMessageInterval"], 42);
        assert_eq!(settings["dynamicMemory"]["maxEntries"], 99);
        assert_eq!(settings["dynamicMemory"]["retrievalLimit"], 7);
        assert_eq!(settings["dynamicMemory"]["contextEnrichmentEnabled"], false);
        assert_eq!(settings["groupDynamicMemory"]["enabled"], true);
    }
}

/// List available backups in downloads directory
#[tauri::command]
pub fn backup_list(app: tauri::AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let downloads = get_downloads_dir()?;
    let mut backups = Vec::new();

    log_info(
        &app,
        "backup",
        format!("Looking for backups in: {:?}", downloads),
    );

    if !downloads.exists() {
        log_info(
            &app,
            "backup",
            format!("Downloads directory does not exist: {:?}", downloads),
        );
        return Ok(backups);
    }

    let read_result = fs::read_dir(&downloads);
    match &read_result {
        Ok(_) => log_info(
            &app,
            "backup",
            "Successfully opened downloads directory".to_string(),
        ),
        Err(e) => log_info(
            &app,
            "backup",
            format!("Failed to read downloads directory: {}", e),
        ),
    }

    for entry in read_result.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))? {
        let entry = entry.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let path = entry.path();

        log_info(&app, "backup", format!("Found file: {:?}", path));

        if let Some(ext) = path.extension() {
            if ext == "lettuce" {
                log_info(&app, "backup", format!("Found .lettuce backup: {:?}", path));
                if let Ok(info) = backup_get_info(app.clone(), path.to_string_lossy().to_string()) {
                    let mut info_obj = info;
                    if let Some(obj) = info_obj.as_object_mut() {
                        obj.insert(
                            "path".to_string(),
                            serde_json::Value::String(path.to_string_lossy().to_string()),
                        );
                        obj.insert(
                            "filename".to_string(),
                            serde_json::Value::String(
                                path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default(),
                            ),
                        );
                    }
                    backups.push(info_obj);
                }
            }
        }
    }

    log_info(
        &app,
        "backup",
        format!("Found {} backups total", backups.len()),
    );

    // Sort by creation date descending
    backups.sort_by(|a, b| {
        let a_time = a.get("createdAt").and_then(|v| v.as_u64()).unwrap_or(0);
        let b_time = b.get("createdAt").and_then(|v| v.as_u64()).unwrap_or(0);
        b_time.cmp(&a_time)
    });

    Ok(backups)
}

/// Delete a backup file
#[tauri::command]
pub fn backup_delete(backup_path: String) -> Result<(), String> {
    fs::remove_file(&backup_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to delete backup: {}", e),
        )
    })
}

/// Get backup info from bytes (for Android content URI support)
#[tauri::command]
pub fn backup_get_info_from_bytes(data: Vec<u8>) -> Result<serde_json::Value, String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = ZipArchive::new(cursor).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read backup archive: {}", e),
        )
    })?;

    // Read manifest
    let mut manifest_file = archive.by_name("manifest.json").map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid backup: missing manifest: {}", e),
        )
    })?;

    let mut manifest_str = String::new();
    manifest_file
        .read_to_string(&mut manifest_str)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let manifest: BackupManifest = serde_json::from_str(&manifest_str).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Invalid manifest: {}", e))
    })?;

    // Count files
    drop(manifest_file);
    let mut total_files = 0;
    let mut image_count = 0;
    let mut avatar_count = 0;
    let mut attachment_count = 0;
    let mut session_attachment_count = 0;
    let mut generated_image_count = 0;

    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name();
            if !file.is_dir() {
                total_files += 1;
                if name.starts_with("images/") {
                    image_count += 1;
                } else if name.starts_with("avatars/") {
                    avatar_count += 1;
                } else if name.starts_with("attachments/") {
                    attachment_count += 1;
                } else if name.starts_with("sessions/") {
                    session_attachment_count += 1;
                } else if name.starts_with("generated_images/") {
                    generated_image_count += 1;
                }
            }
        }
    }

    Ok(serde_json::json!({
        "version": manifest.version,
        "createdAt": manifest.created_at,
        "appVersion": manifest.app_version,
        "encrypted": manifest.encrypted,
        "totalFiles": total_files,
        "imageCount": image_count,
        "avatarCount": avatar_count,
        "attachmentCount": attachment_count,
        "sessionAttachmentCount": session_attachment_count,
        "generatedImageCount": generated_image_count,
    }))
}

/// Check if backup is encrypted from bytes
#[tauri::command]
pub fn backup_check_encrypted_from_bytes(data: Vec<u8>) -> Result<bool, String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = ZipArchive::new(cursor).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read backup archive: {}", e),
        )
    })?;

    let mut manifest_file = archive.by_name("manifest.json").map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid backup: missing manifest: {}", e),
        )
    })?;

    let mut manifest_str = String::new();
    manifest_file
        .read_to_string(&mut manifest_str)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let manifest: BackupManifest = serde_json::from_str(&manifest_str).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Invalid manifest: {}", e))
    })?;

    Ok(manifest.encrypted)
}

/// Verify password for backup from bytes
#[tauri::command]
pub fn backup_verify_password_from_bytes(data: Vec<u8>, password: String) -> Result<bool, String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = ZipArchive::new(cursor).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read backup archive: {}", e),
        )
    })?;

    let mut manifest_file = archive.by_name("manifest.json").map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid backup: missing manifest: {}", e),
        )
    })?;

    let mut manifest_str = String::new();
    manifest_file
        .read_to_string(&mut manifest_str)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let manifest: BackupManifest = serde_json::from_str(&manifest_str).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Invalid manifest: {}", e))
    })?;

    require_encrypted_backup(&manifest)?;

    let salt_b64 = manifest
        .salt
        .ok_or_else(|| "Missing salt in encrypted backup".to_string())?;
    let nonce_b64 = manifest
        .nonce
        .ok_or_else(|| "Missing nonce in encrypted backup".to_string())?;

    let salt_vec = general_purpose::STANDARD
        .decode(&salt_b64)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let nonce_vec = general_purpose::STANDARD
        .decode(&nonce_b64)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    salt.copy_from_slice(&salt_vec);
    nonce.copy_from_slice(&nonce_vec);

    let password =
        require_non_empty_password(Some(password.as_str()), "backup password verification")?;
    let key = derive_key_from_password(password, &salt);

    drop(manifest_file);
    let mut marker_file = archive.by_name("encrypted_marker.bin").map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid backup: missing marker: {}", e),
        )
    })?;

    let mut encrypted_marker = Vec::new();
    marker_file
        .read_to_end(&mut encrypted_marker)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    match decrypt_data(&encrypted_marker, &key, &nonce) {
        Ok(decrypted) => Ok(decrypted == b"LETTUCE_BACKUP_VERIFIED"),
        Err(_) => Ok(false),
    }
}

/// Import backup from bytes (for Android content URI support) - v2 format
#[tauri::command]
pub async fn backup_import_from_bytes(
    app: tauri::AppHandle,
    data: Vec<u8>,
    password: Option<String>,
) -> Result<(), String> {
    let storage = storage_root(&app)?;

    log_info(&app, "backup", "Starting backup import v2 from bytes...");

    // Read manifest
    let manifest: BackupManifest = {
        let cursor = std::io::Cursor::new(&data);
        let mut archive = ZipArchive::new(cursor).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read backup archive: {}", e),
            )
        })?;

        let mut manifest_file = archive.by_name("manifest.json").map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid backup: missing manifest: {}", e),
            )
        })?;

        let mut manifest_str = String::new();
        manifest_file
            .read_to_string(&mut manifest_str)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        serde_json::from_str(&manifest_str).map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Invalid manifest: {}", e))
        })?
    };

    require_encrypted_backup(&manifest)?;

    // Check backup version - only support v2
    if manifest.version < BACKUP_VERSION {
        return Err(format!(
            "Backup version {} is not supported. This app requires backup version {}.",
            manifest.version, BACKUP_VERSION
        ));
    }

    // Prepare encryption params if encrypted
    let encryption_params: Option<([u8; 32], [u8; 24])> = if manifest.encrypted {
        let pwd = require_non_empty_password(password.as_deref(), "backup import from bytes")?;

        let salt_b64 = manifest
            .salt
            .as_ref()
            .ok_or_else(|| "Missing salt".to_string())?;
        let nonce_b64 = manifest
            .nonce
            .as_ref()
            .ok_or_else(|| "Missing nonce".to_string())?;

        let salt_vec = general_purpose::STANDARD
            .decode(salt_b64)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let nonce_vec = general_purpose::STANDARD
            .decode(nonce_b64)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut salt = [0u8; 16];
        let mut nonce = [0u8; 24];
        salt.copy_from_slice(&salt_vec);
        nonce.copy_from_slice(&nonce_vec);

        let key = derive_key_from_password(pwd, &salt);

        // Verify marker BEFORE proceeding - this validates the password
        let cursor = std::io::Cursor::new(&data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut marker_file = archive.by_name("encrypted_marker.bin").map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid backup: missing encryption marker: {}", e),
            )
        })?;

        let mut encrypted_marker = Vec::new();
        marker_file
            .read_to_end(&mut encrypted_marker)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let decrypted = decrypt_data(&encrypted_marker, &key, &nonce)
            .map_err(|_| "Invalid password - decryption failed".to_string())?;

        if decrypted != b"LETTUCE_BACKUP_VERIFIED" {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Invalid password - verification marker mismatch",
            ));
        }

        log_info(&app, "backup", "Password verified successfully");
        Some((key, nonce))
    } else {
        None
    };

    // Helper to read and optionally decrypt a file from the archive (bytes version)
    let read_backup_file_bytes = |data: &[u8],
                                  path: &str,
                                  enc_params: &Option<([u8; 32], [u8; 24])>|
     -> Result<Option<Vec<u8>>, String> {
        let cursor = std::io::Cursor::new(data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        // Try encrypted version first if we have encryption params
        let encrypted_path = format!("{}.enc", path);

        if let Some((ref key, ref nonce)) = enc_params {
            if let Ok(mut file) = archive.by_name(&encrypted_path) {
                let mut contents = Vec::new();
                file.read_to_end(&mut contents)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                let decrypted = decrypt_data(&contents, key, nonce).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to decrypt {}: {}", path, e),
                    )
                })?;
                return Ok(Some(decrypted));
            }
        }

        // Try unencrypted version
        if let Ok(mut file) = archive.by_name(path) {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            return Ok(Some(contents));
        }

        Ok(None)
    };

    log_info(&app, "backup", "Reading JSON data files...");

    // Read all JSON data files
    let meta_data = read_backup_file_bytes(&data, "data/meta.json", &encryption_params)?;
    let settings_data = read_backup_file_bytes(&data, "data/settings.json", &encryption_params)?;
    let provider_credentials_data =
        read_backup_file_bytes(&data, "data/provider_credentials.json", &encryption_params)?;
    let models_data = read_backup_file_bytes(&data, "data/models.json", &encryption_params)?;
    let model_pricing_cache_data =
        read_backup_file_bytes(&data, "data/model_pricing_cache.json", &encryption_params)?;
    let secrets_data = read_backup_file_bytes(&data, "data/secrets.json", &encryption_params)?;
    let prompt_templates_data =
        read_backup_file_bytes(&data, "data/prompt_templates.json", &encryption_params)?;
    let chat_templates_data =
        read_backup_file_bytes(&data, "data/chat_templates.json", &encryption_params)?;
    let personas_data = read_backup_file_bytes(&data, "data/personas.json", &encryption_params)?;
    let characters_data =
        read_backup_file_bytes(&data, "data/characters.json", &encryption_params)?;
    let sessions_data = read_backup_file_bytes(&data, "data/sessions.json", &encryption_params)?;
    let creation_helper_sessions_data = read_backup_file_bytes(
        &data,
        "data/creation_helper_sessions.json",
        &encryption_params,
    )?;
    let group_characters_data =
        read_backup_file_bytes(&data, "data/group_characters.json", &encryption_params)?;
    let group_sessions_data =
        read_backup_file_bytes(&data, "data/group_sessions.json", &encryption_params)?;
    let usage_records_data =
        read_backup_file_bytes(&data, "data/usage_records.json", &encryption_params)?;
    let lorebooks_data = read_backup_file_bytes(&data, "data/lorebooks.json", &encryption_params)?;
    let character_lorebooks_data =
        read_backup_file_bytes(&data, "data/character_lorebooks.json", &encryption_params)?;

    log_info(&app, "backup", "Importing data to database...");

    // Import data in correct order (respecting foreign key constraints)
    if let Some(file_data) = meta_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse meta JSON: {}", e),
            )
        })?;
        import_meta(&app, &json_value)?;
        log_info(&app, "backup", "Meta imported");
    }

    if let Some(file_data) = settings_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse settings JSON: {}", e),
            )
        })?;
        import_settings(&app, &json_value)?;
        log_info(&app, "backup", "Settings imported");
    }

    if let Some(file_data) = provider_credentials_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse provider_credentials JSON: {}", e),
            )
        })?;
        import_provider_credentials(&app, &json_value)?;
        log_info(&app, "backup", "Provider credentials imported");
    }

    if let Some(file_data) = models_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse models JSON: {}", e),
            )
        })?;
        import_models(&app, &json_value)?;
        log_info(&app, "backup", "Models imported");
    }

    if let Some(file_data) = model_pricing_cache_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse model_pricing_cache JSON: {}", e),
            )
        })?;
        import_model_pricing_cache(&app, &json_value)?;
        log_info(&app, "backup", "Model pricing cache imported");
    }

    if let Some(file_data) = secrets_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse secrets JSON: {}", e),
            )
        })?;
        import_secrets(&app, &json_value)?;
        log_info(&app, "backup", "Secrets imported");
    }

    if let Some(file_data) = prompt_templates_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse prompt_templates JSON: {}", e),
            )
        })?;
        import_prompt_templates(&app, &json_value)?;
        log_info(&app, "backup", "Prompt templates imported");
    }

    if let Some(file_data) = personas_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse personas JSON: {}", e),
            )
        })?;
        import_personas(&app, &json_value)?;
        log_info(&app, "backup", "Personas imported");
    }

    if let Some(file_data) = characters_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse characters JSON: {}", e),
            )
        })?;
        import_characters(&app, &json_value)?;
        log_info(&app, "backup", "Characters imported");
    }

    if let Some(file_data) = chat_templates_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse chat_templates JSON: {}", e),
            )
        })?;
        import_chat_templates(&app, &json_value)?;
        log_info(&app, "backup", "Chat templates imported");
    }

    if let Some(file_data) = sessions_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse sessions JSON: {}", e),
            )
        })?;
        import_sessions(&app, &json_value)?;
        log_info(&app, "backup", "Sessions imported");
    }

    if let Some(file_data) = creation_helper_sessions_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse creation_helper_sessions JSON: {}", e),
            )
        })?;
        import_creation_helper_sessions(&app, &json_value)?;
        log_info(&app, "backup", "Creation helper sessions imported");
    }

    if let Some(file_data) = group_characters_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse group_characters JSON: {}", e),
            )
        })?;
        import_group_characters(&app, &json_value)?;
        log_info(&app, "backup", "Group character presets imported");
    }

    if let Some(file_data) = group_sessions_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse group sessions JSON: {}", e),
            )
        })?;
        import_group_sessions(&app, &json_value)?;
        log_info(&app, "backup", "Group sessions imported");
    }

    if let Some(file_data) = usage_records_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse usage_records JSON: {}", e),
            )
        })?;
        import_usage_records(&app, &json_value)?;
        log_info(&app, "backup", "Usage records imported");
    }

    // Lorebooks (no dependencies, import before character_lorebooks)
    if let Some(file_data) = lorebooks_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse lorebooks JSON: {}", e),
            )
        })?;
        import_lorebooks(&app, &json_value)?;
        log_info(&app, "backup", "Lorebooks imported");
    }

    // Character-lorebook links (depends on characters and lorebooks)
    if let Some(file_data) = character_lorebooks_data {
        let json_str = String::from_utf8(file_data)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse character_lorebooks JSON: {}", e),
            )
        })?;
        import_character_lorebooks(&app, &json_value)?;
        log_info(&app, "backup", "Character-lorebook links imported");
    }

    log_info(&app, "backup", "Extracting media files...");

    // Extract media files to staging directory
    let staging_dir = storage.join(".import_staging");
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    fs::create_dir_all(&staging_dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Extract media files (images, avatars, attachments)
    let cursor = std::io::Cursor::new(&data);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let file_name = file.name().to_string();
        if !archive_name_targets_media(&file_name) {
            continue;
        }

        let relative_path = sanitize_media_archive_name(&file_name, false)?;

        if file.is_dir() {
            let outpath = staging_dir.join(&relative_path);
            fs::create_dir_all(&outpath)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        } else {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            // Decrypt if needed
            let (outpath, final_contents) = if let Some((ref key, ref nonce)) = encryption_params {
                if file_name.ends_with(".enc") {
                    let decrypted = decrypt_data(&contents, key, nonce).map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Failed to decrypt {}: {}", file_name, e),
                        )
                    })?;
                    let out_name = sanitize_media_archive_name(&file_name, true)?;
                    (staging_dir.join(out_name), decrypted)
                } else {
                    (staging_dir.join(&relative_path), contents)
                }
            } else {
                (staging_dir.join(&relative_path), contents)
            };

            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }

            let mut outfile = File::create(&outpath)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            outfile
                .write_all(&final_contents)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }

    // Copy media from staging to actual locations
    let images_dir = storage.join("images");
    let avatars_dir = storage.join("avatars");
    let attachments_dir = storage.join("attachments");
    let sessions_dir = storage.join("sessions");
    let generated_images_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .join("generated_images");

    // Clear existing media directories
    if images_dir.exists() {
        fs::remove_dir_all(&images_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if avatars_dir.exists() {
        fs::remove_dir_all(&avatars_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if attachments_dir.exists() {
        fs::remove_dir_all(&attachments_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    let staged_images = staging_dir.join("images");
    if staged_images.exists() {
        copy_dir_all(&staged_images, &images_dir)?;
        log_info(&app, "backup", "Images restored");
    }

    let staged_avatars = staging_dir.join("avatars");
    if staged_avatars.exists() {
        copy_dir_all(&staged_avatars, &avatars_dir)?;
        log_info(&app, "backup", "Avatars restored");
    }

    let staged_attachments = staging_dir.join("attachments");
    if staged_attachments.exists() {
        copy_dir_all(&staged_attachments, &attachments_dir)?;
        log_info(&app, "backup", "Attachments restored");
    }

    let staged_sessions = staging_dir.join("sessions");
    if staged_sessions.exists() {
        if sessions_dir.exists() {
            fs::remove_dir_all(&sessions_dir)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
        copy_dir_all(&staged_sessions, &sessions_dir)?;
        log_info(&app, "backup", "Session attachments restored");
    }

    let staged_generated_images = staging_dir.join("generated_images");
    if staged_generated_images.exists() {
        if generated_images_dir.exists() {
            fs::remove_dir_all(&generated_images_dir)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
        copy_dir_all(&staged_generated_images, &generated_images_dir)?;
        log_info(&app, "backup", "Generated images restored");
    }

    // Cleanup staging
    fs::remove_dir_all(&staging_dir).ok();

    // Emit event to notify frontend to reload
    app.emit("database-reloaded", ()).ok();

    log_info(&app, "backup", "Backup import v2 from bytes complete!");

    Ok(())
}

/// Check if a backup contains characters with dynamic memory enabled (v2 format)
#[tauri::command]
pub async fn backup_check_dynamic_memory(
    app: tauri::AppHandle,
    backup_path: String,
    password: Option<String>,
) -> Result<bool, String> {
    use crate::utils::log_info;

    log_info(
        &app,
        "backup_check_dynamic_memory",
        format!("Checking backup at: {}", backup_path),
    );

    let file = open_backup_file(&app, &backup_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read backup archive: {}", e),
        )
    })?;

    // Read manifest to check if encrypted
    let manifest: BackupManifest = {
        let mut manifest_file = archive.by_name("manifest.json").map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid backup: missing manifest: {}", e),
            )
        })?;

        let mut manifest_str = String::new();
        manifest_file
            .read_to_string(&mut manifest_str)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        serde_json::from_str(&manifest_str).map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Invalid manifest: {}", e))
        })?
    };

    log_info(
        &app,
        "backup_check_dynamic_memory",
        format!(
            "Manifest encrypted: {}, version: {}",
            manifest.encrypted, manifest.version
        ),
    );

    require_encrypted_backup(&manifest)?;

    // Prepare encryption params if needed
    let encryption_params: Option<([u8; 32], [u8; 24])> = if manifest.encrypted {
        let pwd = require_non_empty_password(password.as_deref(), "backup inspection")?;

        let salt_b64 = manifest
            .salt
            .as_ref()
            .ok_or_else(|| "Missing salt".to_string())?;
        let nonce_b64 = manifest
            .nonce
            .as_ref()
            .ok_or_else(|| "Missing nonce".to_string())?;

        let salt_vec = general_purpose::STANDARD
            .decode(salt_b64)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let nonce_vec = general_purpose::STANDARD
            .decode(nonce_b64)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut salt = [0u8; 16];
        let mut nonce = [0u8; 24];
        salt.copy_from_slice(&salt_vec);
        nonce.copy_from_slice(&nonce_vec);

        let key = derive_key_from_password(pwd, &salt);
        Some((key, nonce))
    } else {
        None
    };

    // Read the characters JSON file - path depends on encryption (v2 format)
    let json_path = if manifest.encrypted {
        "data/characters.json.enc"
    } else {
        "data/characters.json"
    };

    log_info(
        &app,
        "backup_check_dynamic_memory",
        format!("Looking for characters at: {}", json_path),
    );

    let mut json_file = archive.by_name(json_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid backup: missing characters at {}: {}", json_path, e),
        )
    })?;

    let mut json_data = Vec::new();
    json_file
        .read_to_end(&mut json_data)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    log_info(
        &app,
        "backup_check_dynamic_memory",
        format!("Read {} bytes of characters JSON", json_data.len()),
    );

    // Decrypt if needed
    let final_json_data = if let Some((key, nonce)) = encryption_params {
        log_info(
            &app,
            "backup_check_dynamic_memory",
            "Decrypting characters JSON...".to_string(),
        );
        decrypt_data(&json_data, &key, &nonce)?
    } else {
        json_data
    };

    // Parse JSON and check for dynamic memory
    let json_str = String::from_utf8(final_json_data)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let characters: Vec<serde_json::Value> = serde_json::from_str(&json_str).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse characters JSON: {}", e),
        )
    })?;

    let dynamic_count = characters
        .iter()
        .filter(|c| c.get("memory_type").and_then(|v| v.as_str()) == Some("dynamic"))
        .count();

    log_info(
        &app,
        "backup_check_dynamic_memory",
        format!("Found {} characters with dynamic memory", dynamic_count),
    );

    Ok(dynamic_count > 0)
}

/// Check if a backup (from bytes) contains characters with dynamic memory enabled (v2 format)
#[tauri::command]
pub async fn backup_check_dynamic_memory_from_bytes(
    app: tauri::AppHandle,
    data: Vec<u8>,
    password: Option<String>,
) -> Result<bool, String> {
    use crate::utils::log_info;
    use std::io::Cursor;

    log_info(
        &app,
        "backup_check_dynamic_memory_from_bytes",
        format!("Checking backup from bytes ({} bytes)", data.len()),
    );

    let cursor = Cursor::new(&data);
    let mut archive = ZipArchive::new(cursor).map_err(|e| {
        log_info(
            &app,
            "backup_check_dynamic_memory_from_bytes",
            format!("Failed to read archive: {}", e),
        );
        format!("Failed to read backup archive: {}", e)
    })?;

    log_info(
        &app,
        "backup_check_dynamic_memory_from_bytes",
        "Successfully opened archive".to_string(),
    );

    // Read manifest to check if encrypted
    let manifest: BackupManifest = {
        let mut manifest_file = archive.by_name("manifest.json").map_err(|e| {
            log_info(
                &app,
                "backup_check_dynamic_memory_from_bytes",
                format!("Failed to read manifest: {}", e),
            );
            format!("Invalid backup: missing manifest: {}", e)
        })?;

        let mut manifest_str = String::new();
        manifest_file
            .read_to_string(&mut manifest_str)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        serde_json::from_str(&manifest_str).map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Invalid manifest: {}", e))
        })?
    };

    log_info(
        &app,
        "backup_check_dynamic_memory_from_bytes",
        format!(
            "Manifest encrypted: {}, version: {}",
            manifest.encrypted, manifest.version
        ),
    );

    require_encrypted_backup(&manifest)?;

    // Prepare encryption params if needed
    let encryption_params: Option<([u8; 32], [u8; 24])> = if manifest.encrypted {
        let pwd = require_non_empty_password(password.as_deref(), "backup inspection")?;

        let salt_b64 = manifest
            .salt
            .as_ref()
            .ok_or_else(|| "Missing salt".to_string())?;
        let nonce_b64 = manifest
            .nonce
            .as_ref()
            .ok_or_else(|| "Missing nonce".to_string())?;

        let salt_vec = general_purpose::STANDARD
            .decode(salt_b64)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let nonce_vec = general_purpose::STANDARD
            .decode(nonce_b64)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut salt = [0u8; 16];
        let mut nonce = [0u8; 24];
        salt.copy_from_slice(&salt_vec);
        nonce.copy_from_slice(&nonce_vec);

        let key = derive_key_from_password(pwd, &salt);
        Some((key, nonce))
    } else {
        None
    };

    // Read the characters JSON file - path depends on encryption (v2 format)
    let json_path = if manifest.encrypted {
        "data/characters.json.enc"
    } else {
        "data/characters.json"
    };

    log_info(
        &app,
        "backup_check_dynamic_memory_from_bytes",
        format!("Looking for characters at: {}", json_path),
    );

    // Re-open archive to read characters file
    let cursor = Cursor::new(&data);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut json_file = archive.by_name(json_path).map_err(|e| {
        log_info(
            &app,
            "backup_check_dynamic_memory_from_bytes",
            format!("Failed to find characters at {}: {}", json_path, e),
        );
        format!("Invalid backup: missing characters at {}: {}", json_path, e)
    })?;

    let mut json_data = Vec::new();
    json_file
        .read_to_end(&mut json_data)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    log_info(
        &app,
        "backup_check_dynamic_memory_from_bytes",
        format!("Read {} bytes of characters JSON", json_data.len()),
    );

    // Decrypt if needed
    let final_json_data = if let Some((key, nonce)) = encryption_params {
        log_info(
            &app,
            "backup_check_dynamic_memory_from_bytes",
            "Decrypting characters JSON...".to_string(),
        );
        decrypt_data(&json_data, &key, &nonce)?
    } else {
        json_data
    };

    // Parse JSON and check for dynamic memory
    let json_str = String::from_utf8(final_json_data)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let characters: Vec<serde_json::Value> = serde_json::from_str(&json_str).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse characters JSON: {}", e),
        )
    })?;

    let dynamic_count = characters
        .iter()
        .filter(|c| c.get("memory_type").and_then(|v| v.as_str()) == Some("dynamic"))
        .count();

    log_info(
        &app,
        "backup_check_dynamic_memory_from_bytes",
        format!("Found {} characters with dynamic memory", dynamic_count),
    );

    Ok(dynamic_count > 0)
}

/// Disable dynamic memory for all characters
/// This is called after importing a backup when the user doesn't want to download the embedding model
#[tauri::command]
pub async fn backup_disable_dynamic_memory(app: tauri::AppHandle) -> Result<(), String> {
    log_info(
        &app,
        "backup",
        "Disabling dynamic memory for all characters...",
    );

    let conn = open_db(&app)?;

    // Update all characters to use manual memory
    conn.execute(
        "UPDATE characters SET memory_type = 'manual' WHERE memory_type = 'dynamic'",
        [],
    )
    .map_err(|e| {
        log_info(
            &app,
            "backup",
            format!("Failed to disable dynamic memory: {}", e),
        );
        e.to_string()
    })?;

    let affected = conn.changes();

    log_info(
        &app,
        "backup",
        format!("Updated {} characters to manual memory", affected),
    );

    // Update global settings to disable dynamic memory
    let advanced_settings_json: Option<String> = conn
        .query_row(
            "SELECT advanced_settings FROM settings WHERE id = 1",
            [],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .flatten();

    // Handle the case where advanced_settings is NULL or doesn't exist
    let current_settings = if let Some(json_str) = advanced_settings_json {
        serde_json::from_str::<serde_json::Value>(&json_str)
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let mut settings = current_settings;
    disable_dynamic_memory_in_advanced_settings(&mut settings);

    let new_json = serde_json::to_string(&settings)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        "UPDATE settings SET advanced_settings = ? WHERE id = 1",
        [&new_json],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    log_info(&app, "backup", "Disabled dynamic memory in global settings");

    // Reload database to ensure frontend gets updated data
    super::db::reload_database(&app)?;

    Ok(())
}
