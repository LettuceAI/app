use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use machine_uid::get as get_machine_uid;
use rand::rngs::OsRng;
use rand::RngCore;
use std::fs;
use std::path::PathBuf;

pub use crate::utils::ensure_lettuce_dir;
// log helpers not used here

const SETTINGS_FILE: &str = "settings.bin";
const CHARACTERS_FILE: &str = "characters.bin";
const PERSONAS_FILE: &str = "personas.bin";
const SESSIONS_DIR: &str = "sessions";
const SESSIONS_INDEX_FILE: &str = "sessions/index.bin";

pub fn storage_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    ensure_lettuce_dir(app)
}

#[tauri::command]
pub fn get_storage_root(app: tauri::AppHandle) -> Result<String, String> {
    storage_root(&app).map(|p| p.to_string_lossy().to_string())
}

pub fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(storage_root(app)?.join(SETTINGS_FILE))
}

pub fn characters_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(storage_root(app)?.join(CHARACTERS_FILE))
}

pub fn personas_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(storage_root(app)?.join(PERSONAS_FILE))
}

pub fn sessions_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(storage_root(app)?.join(SESSIONS_DIR))
}

pub fn sessions_index_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(storage_root(app)?.join(SESSIONS_INDEX_FILE))
}

fn derive_key() -> Result<[u8; 32], String> {
    let machine_id = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            get_machine_uid().unwrap_or_else(|_| {
                format!(
                    "{}|{}|{}",
                    whoami::username(),
                    whoami::fallible::hostname().unwrap_or_else(|_| "unknown-host".into()),
                    std::env::consts::OS
                )
            })
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            format!(
                "{}|{}|{}",
                whoami::username(),
                whoami::fallible::hostname().unwrap_or_else(|_| "unknown-host".into()),
                std::env::consts::OS
            )
        }
    };
    let mut hasher = blake3::Hasher::new();
    hasher.update(machine_id.as_bytes());
    hasher.update(b"lettuceai.storage.v1");
    let hash = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(hash.as_bytes());
    Ok(key)
}

#[allow(dead_code)]
fn encrypt(content: &[u8]) -> Result<Vec<u8>, String> {
    let key = derive_key()?;
    let cipher = XChaCha20Poly1305::new(&key.into());
    let mut nonce_bytes = [0u8; 24];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from(nonce_bytes);
    let mut out = Vec::with_capacity(24 + content.len() + 16);
    let ciphertext = cipher
        .encrypt(&nonce, content)
        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("encrypt: {e}")))?;
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

fn decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 24 {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "corrupted data",
        ));
    }
    let (nonce_bytes, ciphertext) = data.split_at(24);
    let key = derive_key()?;
    let cipher = XChaCha20Poly1305::new(&key.into());
    let nonce = XNonce::from(*<&[u8; 24]>::try_from(nonce_bytes).map_err(|_| "invalid nonce")?);
    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("decrypt: {e}")))
}

pub fn read_encrypted_file(path: &PathBuf) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes =
        fs::read(path).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if bytes.is_empty() {
        return Ok(None);
    }
    let decrypted = decrypt(&bytes)?;
    let text = String::from_utf8(decrypted)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

#[allow(dead_code)]
pub fn write_encrypted_file(path: &PathBuf, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    let bytes = encrypt(content.as_bytes())?;
    fs::write(path, bytes).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[allow(dead_code)]
pub fn delete_file_if_exists(path: &PathBuf) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    Ok(())
}
