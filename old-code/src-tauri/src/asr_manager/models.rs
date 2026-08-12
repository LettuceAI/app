use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

const WHISPER_REPO: &str = "ggerganov/whisper.cpp";
const WHISPER_RESOLVE_BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
const WHISPER_MODEL_API: &str = "https://huggingface.co/api/models/ggerganov/whisper.cpp";

#[derive(Debug, Deserialize)]
struct HfModelDetail {
    #[serde(default)]
    siblings: Vec<HfSibling>,
}

#[derive(Debug, Deserialize)]
struct HfSibling {
    rfilename: String,
}

#[derive(Debug, Deserialize)]
struct HfTreeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    path: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    lfs: Option<HfLfsInfo>,
}

#[derive(Debug, Deserialize)]
struct HfLfsInfo {
    #[serde(default)]
    size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrWhisperModelPreset {
    pub id: String,
    pub filename: String,
    pub label: String,
    pub repo: String,
    pub download_url: String,
    pub size_bytes: u64,
    pub english_only: bool,
    pub quantized: bool,
    pub recommended: bool,
    pub recommended_for_mobile: bool,
    pub recommended_for_desktop: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrInstalledWhisperModel {
    pub id: String,
    pub filename: String,
    pub label: String,
    pub path: String,
    pub size_bytes: u64,
    pub english_only: bool,
    pub quantized: bool,
}

fn whisper_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let lettuce_dir = crate::infra::utils::ensure_lettuce_dir(app)?;
    let dir = lettuce_dir.join("models").join("whisper");
    std::fs::create_dir_all(&dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(dir)
}

fn file_name_string(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn derive_id_from_filename(filename: &str) -> String {
    filename
        .strip_prefix("ggml-")
        .unwrap_or(filename)
        .strip_suffix(".bin")
        .unwrap_or(filename)
        .to_string()
}

fn derive_filename_from_id(model_id: &str) -> String {
    format!("ggml-{}.bin", model_id)
}

fn safe_folder_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '-',
        })
        .collect()
}

fn is_whisper_model_bin(filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    lower.starts_with("ggml-")
        && lower.ends_with(".bin")
        && !lower.contains("encoder")
        && !lower.contains(".mlmodelc")
}

fn is_english_only(model_id: &str) -> bool {
    model_id.contains(".en")
}

fn is_quantized(model_id: &str) -> bool {
    model_id.contains("-q")
}

fn is_recommended(model_id: &str) -> bool {
    matches!(
        model_id,
        "base.en"
            | "base"
            | "small.en"
            | "small"
            | "medium.en-q5_0"
            | "medium-q5_0"
            | "large-v3-turbo-q5_0"
    )
}

fn is_recommended_for_mobile(model_id: &str) -> bool {
    matches!(
        model_id,
        "tiny.en"
            | "tiny"
            | "base.en-q5_1"
            | "base-q5_1"
            | "base.en"
            | "base"
            | "small.en-q5_1"
            | "small-q5_1"
    )
}

fn is_recommended_for_desktop(model_id: &str) -> bool {
    matches!(
        model_id,
        "small.en"
            | "small"
            | "medium.en-q5_0"
            | "medium-q5_0"
            | "medium.en"
            | "medium"
            | "large-v3-turbo-q5_0"
            | "large-v3-turbo"
    )
}

fn format_model_label(model_id: &str) -> String {
    model_id
        .split(['-', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part.starts_with('q') {
                part.to_ascii_uppercase()
            } else if part == "en" {
                "English".to_string()
            } else if part == "tdrz" {
                "TDRZ".to_string()
            } else if part == "v1" || part == "v2" || part == "v3" {
                part.to_ascii_uppercase()
            } else {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn validate_model_id(model_id: &str) -> Result<(), String> {
    let trimmed = model_id.trim();
    if trimmed.is_empty() {
        return Err("Whisper model id cannot be empty".to_string());
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err(format!("Unsupported Whisper model id: {}", model_id));
    }
    Ok(())
}

async fn fetch_remote_whisper_models() -> Result<Vec<AsrWhisperModelPreset>, String> {
    let client = reqwest::Client::builder()
        .user_agent("LettuceAI/1.0")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), e.to_string()))?;

    let detail_resp = client.get(WHISPER_MODEL_API).send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to fetch Whisper model detail: {}", e),
        )
    })?;
    if !detail_resp.status().is_success() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Failed to fetch Whisper model detail: HTTP {}",
                detail_resp.status()
            ),
        ));
    }

    let detail = detail_resp.json::<HfModelDetail>().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse Whisper model detail: {}", e),
        )
    })?;

    let tree_resp = client
        .get(format!("{WHISPER_MODEL_API}/tree/main?recursive=false"))
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to fetch Whisper file tree: {}", e),
            )
        })?;
    if !tree_resp.status().is_success() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Failed to fetch Whisper file tree: HTTP {}",
                tree_resp.status()
            ),
        ));
    }

    let tree_entries = tree_resp.json::<Vec<HfTreeEntry>>().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse Whisper file tree: {}", e),
        )
    })?;
    let size_map = tree_entries
        .into_iter()
        .filter(|entry| entry.entry_type == "file")
        .map(|entry| {
            (
                entry.path,
                entry
                    .lfs
                    .as_ref()
                    .map(|value| value.size)
                    .unwrap_or(entry.size),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut results = detail
        .siblings
        .into_iter()
        .filter_map(|sibling| {
            if !is_whisper_model_bin(&sibling.rfilename) {
                return None;
            }
            let id = derive_id_from_filename(&sibling.rfilename);
            Some(AsrWhisperModelPreset {
                id: id.clone(),
                filename: sibling.rfilename.clone(),
                label: format_model_label(&id),
                repo: WHISPER_REPO.to_string(),
                download_url: format!("{WHISPER_RESOLVE_BASE}/{}", sibling.rfilename),
                size_bytes: size_map.get(&sibling.rfilename).copied().unwrap_or(0),
                english_only: is_english_only(&id),
                quantized: is_quantized(&id),
                recommended: is_recommended(&id),
                recommended_for_mobile: is_recommended_for_mobile(&id),
                recommended_for_desktop: is_recommended_for_desktop(&id),
            })
        })
        .collect::<Vec<_>>();

    results.sort_by(|left, right| left.filename.cmp(&right.filename));
    Ok(results)
}

#[tauri::command]
pub async fn asr_whisper_list_available_models() -> Result<Vec<AsrWhisperModelPreset>, String> {
    fetch_remote_whisper_models().await
}

#[tauri::command]
pub fn asr_whisper_get_models_dir(app: AppHandle) -> Result<String, String> {
    Ok(whisper_models_dir(&app)?.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn asr_whisper_queue_model_download(
    app: AppHandle,
    model_id: String,
) -> Result<String, String> {
    validate_model_id(&model_id)?;
    let filename = derive_filename_from_id(model_id.trim());
    let model_dir = whisper_models_dir(&app)?.join(safe_folder_name(model_id.trim()));
    std::fs::create_dir_all(&model_dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let destination_path = model_dir.join(&filename);

    let metadata = crate::hf_browser::QueueDownloadMetadata {
        create_model_when_finished: false,
        mmproj_file: crate::hf_browser::MmprojFileLink::Disabled(false),
        mtp_file: crate::hf_browser::MmprojFileLink::Disabled(false),
        mtp_bundled: false,
        install_id: Some(format!("whisper-{}", model_id.trim())),
        display_name: Some(format_model_label(model_id.trim())),
        context_length: None,
        kv_type: None,
        llama_offload_kqv: None,
        llama_gpu_layers: None,
        llama_model_offload_mode: None,
        download_role: Some("model".to_string()),
        queue_kind: Some("whisper".to_string()),
        asset_root: Some(model_dir.to_string_lossy().to_string()),
        install_kind: Some("model".to_string()),
        variant: Some(model_id.trim().to_string()),
        voice_id: None,
        download_url: Some(format!("{WHISPER_RESOLVE_BASE}/{}", filename)),
        destination_path: Some(destination_path.to_string_lossy().to_string()),
        expected_size: None,
        sha256: None,
        runtime_release: None,
        runtime_asset: None,
        force_redownload: false,
        lora_keywords: None,
        lora_base_model: None,
    };

    crate::hf_browser::hf_queue_download(app, WHISPER_REPO.to_string(), filename, Some(metadata))
        .await
}

#[tauri::command]
pub async fn asr_whisper_list_installed_models(
    app: AppHandle,
) -> Result<Vec<AsrInstalledWhisperModel>, String> {
    let models_dir = whisper_models_dir(&app)?;
    let mut results = Vec::new();
    let entries = std::fs::read_dir(&models_dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let files = match std::fs::read_dir(&path) {
            Ok(files) => files,
            Err(_) => continue,
        };
        for file_entry in files.flatten() {
            let file_path = file_entry.path();
            if !file_path.is_file() {
                continue;
            }

            let filename = file_name_string(&file_path);
            if !is_whisper_model_bin(&filename) || filename.ends_with(".tmp") {
                continue;
            }

            let id = derive_id_from_filename(&filename);
            let size_bytes = file_entry.metadata().map(|value| value.len()).unwrap_or(0);
            results.push(AsrInstalledWhisperModel {
                id: id.clone(),
                filename: filename.clone(),
                label: format_model_label(&id),
                path: file_path.to_string_lossy().to_string(),
                size_bytes,
                english_only: is_english_only(&id),
                quantized: is_quantized(&id),
            });
        }
    }

    results.sort_by(|left, right| left.filename.cmp(&right.filename));
    Ok(results)
}

#[tauri::command]
pub async fn asr_whisper_delete_installed_model(
    app: AppHandle,
    file_path: String,
) -> Result<(), String> {
    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Ok(());
    }

    let models_dir = whisper_models_dir(&app)?;
    if !path.starts_with(&models_dir) {
        return Err("Cannot delete files outside the Whisper models directory".to_string());
    }

    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(parent) = path.parent() {
        if parent != models_dir {
            let is_empty = std::fs::read_dir(parent)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
            if is_empty {
                let _ = tokio::fs::remove_dir(parent).await;
            }
        }
    }

    Ok(())
}
