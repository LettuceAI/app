use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

use super::engine::{kokoro_platform_allows_variant, KokoroModelVariant};
use super::model::KokoroError;
use super::voices::list_installed_voice_ids;

const HF_MODEL_REPO: &str = "onnx-community/Kokoro-82M-v1.0-ONNX";
const HF_RESOLVE_BASE: &str =
    "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/main";
const HF_MODEL_API: &str = "https://huggingface.co/api/models/onnx-community/Kokoro-82M-v1.0-ONNX";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroAvailableVoice {
    pub id: String,
    pub installed: bool,
}

#[derive(Debug, Clone)]
struct DownloadFileSpec {
    remote_path: &'static str,
    local_relative_path: &'static str,
    filename: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroQueuedInstall {
    pub install_id: String,
    pub queue_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HuggingFaceModelResponse {
    siblings: Option<Vec<HuggingFaceSibling>>,
}

#[derive(Debug, Deserialize)]
struct HuggingFaceSibling {
    rfilename: String,
}

pub fn default_asset_root(app: &AppHandle) -> Result<PathBuf, KokoroError> {
    let lettuce_dir = crate::infra::utils::ensure_lettuce_dir(app)
        .map_err(|message| KokoroError::Io(std::io::Error::other(message)))?;
    let root = lettuce_dir.join("kokoro");
    fs::create_dir_all(&root)?;
    Ok(root)
}

pub async fn queue_model_install(
    app: AppHandle,
    asset_root: PathBuf,
    variant: KokoroModelVariant,
) -> Result<KokoroQueuedInstall, KokoroError> {
    if !kokoro_platform_allows_variant(variant) {
        return Err(KokoroError::UnsupportedVariant(format!(
            "{} is not allowed on this platform",
            variant.id()
        )));
    }

    let install_id = uuid::Uuid::new_v4().to_string();
    let plan = model_download_plan(variant);
    let queue_ids = queue_download_plan(
        &app,
        &asset_root,
        &install_id,
        &plan,
        "model",
        Some(variant.id()),
        None,
        Some(format!("Kokoro {}", variant.id())),
    )
    .await?;

    Ok(KokoroQueuedInstall {
        install_id,
        queue_ids,
    })
}

pub async fn queue_voice_install(
    app: AppHandle,
    asset_root: PathBuf,
    voice_id: String,
) -> Result<KokoroQueuedInstall, KokoroError> {
    if !is_valid_voice_id(&voice_id) {
        return Err(KokoroError::VoiceParse(format!(
            "Unsupported voice id: {}",
            voice_id
        )));
    }

    let install_id = uuid::Uuid::new_v4().to_string();
    let leaked_remote = Box::leak(format!("voices/{voice_id}.bin").into_boxed_str());
    let leaked_local = Box::leak(format!("voices/{voice_id}.bin").into_boxed_str());
    let leaked_filename = Box::leak(format!("{voice_id}.bin").into_boxed_str());
    let plan = vec![DownloadFileSpec {
        remote_path: leaked_remote,
        local_relative_path: leaked_local,
        filename: leaked_filename,
    }];

    let queue_ids = queue_download_plan(
        &app,
        &asset_root,
        &install_id,
        &plan,
        "voice",
        None,
        Some(voice_id.clone()),
        Some(format!("Kokoro voice {}", voice_id)),
    )
    .await?;

    Ok(KokoroQueuedInstall {
        install_id,
        queue_ids,
    })
}

pub async fn list_available_voices(
    asset_root: &Path,
) -> Result<Vec<KokoroAvailableVoice>, KokoroError> {
    let installed = list_installed_voice_ids(asset_root)?
        .into_iter()
        .collect::<HashSet<_>>();
    let client = reqwest::Client::new();
    let response =
        client.get(HF_MODEL_API).send().await.map_err(|err| {
            KokoroError::Config(format!("Failed to query {HF_MODEL_REPO}: {err}"))
        })?;
    if !response.status().is_success() {
        return Err(KokoroError::Config(format!(
            "Failed to query {HF_MODEL_REPO}: HTTP {}",
            response.status()
        )));
    }

    let payload = response
        .json::<HuggingFaceModelResponse>()
        .await
        .map_err(|err| KokoroError::Config(format!("Invalid HF model response: {err}")))?;

    let mut voices = payload
        .siblings
        .unwrap_or_default()
        .into_iter()
        .filter_map(|sibling| {
            sibling
                .rfilename
                .strip_prefix("voices/")
                .map(str::to_string)
        })
        .filter_map(|filename| filename.strip_suffix(".bin").map(str::to_string))
        .filter(|voice_id| is_valid_voice_id(voice_id))
        .map(|id| KokoroAvailableVoice {
            installed: installed.contains(&id),
            id,
        })
        .collect::<Vec<_>>();

    voices.sort_by(|left, right| left.id.cmp(&right.id));
    voices.dedup_by(|left, right| left.id == right.id);
    Ok(voices)
}

fn model_download_plan(variant: KokoroModelVariant) -> Vec<DownloadFileSpec> {
    let model_filename = match variant {
        KokoroModelVariant::Fp32 => "model.onnx",
        KokoroModelVariant::Fp16 => "model_fp16.onnx",
        KokoroModelVariant::Int8 => "model_quantized.onnx",
    };

    vec![
        DownloadFileSpec {
            remote_path: "config.json",
            local_relative_path: "config.json",
            filename: "config.json",
        },
        DownloadFileSpec {
            remote_path: "tokenizer.json",
            local_relative_path: "tokenizer.json",
            filename: "tokenizer.json",
        },
        DownloadFileSpec {
            remote_path: "tokenizer_config.json",
            local_relative_path: "tokenizer_config.json",
            filename: "tokenizer_config.json",
        },
        DownloadFileSpec {
            remote_path: Box::leak(format!("onnx/{model_filename}").into_boxed_str()),
            local_relative_path: Box::leak(format!("onnx/{model_filename}").into_boxed_str()),
            filename: model_filename,
        },
    ]
}

fn is_valid_voice_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

async fn queue_download_plan(
    app: &AppHandle,
    asset_root: &Path,
    install_id: &str,
    plan: &[DownloadFileSpec],
    install_kind: &str,
    variant: Option<&str>,
    voice_id: Option<String>,
    display_name: Option<String>,
) -> Result<Vec<String>, KokoroError> {
    fs::create_dir_all(asset_root)?;
    let asset_root_str = asset_root.to_string_lossy().to_string();
    let mut queue_ids = Vec::with_capacity(plan.len());

    for spec in plan {
        let destination_path = asset_root.join(spec.local_relative_path);
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let metadata = crate::hf_browser::QueueDownloadMetadata {
            create_model_when_finished: false,
            mmproj_file: crate::hf_browser::MmprojFileLink::Disabled(false),
            install_id: Some(install_id.to_string()),
            display_name: display_name.clone(),
            context_length: None,
            kv_type: None,
            download_role: Some(install_kind.to_string()),
            queue_kind: Some("kokoro".to_string()),
            asset_root: Some(asset_root_str.clone()),
            install_kind: Some(install_kind.to_string()),
            variant: variant.map(str::to_string),
            voice_id: voice_id.clone(),
            download_url: Some(format!("{HF_RESOLVE_BASE}/{}", spec.remote_path)),
            destination_path: Some(destination_path.to_string_lossy().to_string()),
            force_redownload: true,
        };

        let queue_id = crate::hf_browser::hf_queue_download(
            app.clone(),
            HF_MODEL_REPO.to_string(),
            spec.filename.to_string(),
            Some(metadata),
        )
        .await
        .map_err(KokoroError::Config)?;

        queue_ids.push(queue_id);
    }

    Ok(queue_ids)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroStorageStats {
    pub model_bytes: u64,
    pub voices_bytes: u64,
    pub total_bytes: u64,
    pub voice_count: u32,
}

fn dir_total_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

fn count_voice_bins(path: &Path) -> u32 {
    if !path.exists() {
        return 0;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().map(|ext| ext == "bin").unwrap_or(false))
        .count() as u32
}

pub fn storage_stats(asset_root: &Path) -> KokoroStorageStats {
    let model_bytes = dir_total_size(&asset_root.join("onnx"));
    let voices_dir = asset_root.join("voices");
    let voices_bytes = dir_total_size(&voices_dir);
    let voice_count = count_voice_bins(&voices_dir);
    KokoroStorageStats {
        model_bytes,
        voices_bytes,
        total_bytes: model_bytes + voices_bytes,
        voice_count,
    }
}

pub async fn queue_voices_install(
    app: AppHandle,
    asset_root: PathBuf,
    voice_ids: Vec<String>,
) -> Result<KokoroQueuedInstall, KokoroError> {
    if voice_ids.is_empty() {
        return Err(KokoroError::VoiceParse("No voices to install".into()));
    }
    for id in &voice_ids {
        if !is_valid_voice_id(id) {
            return Err(KokoroError::VoiceParse(format!("Unsupported voice id: {id}")));
        }
    }

    let install_id = uuid::Uuid::new_v4().to_string();
    let plan: Vec<DownloadFileSpec> = voice_ids
        .iter()
        .map(|id| {
            let leaked_remote = Box::leak(format!("voices/{id}.bin").into_boxed_str());
            let leaked_local = Box::leak(format!("voices/{id}.bin").into_boxed_str());
            let leaked_filename = Box::leak(format!("{id}.bin").into_boxed_str());
            DownloadFileSpec {
                remote_path: leaked_remote,
                local_relative_path: leaked_local,
                filename: leaked_filename,
            }
        })
        .collect();

    let display = if voice_ids.len() == 1 {
        format!("Kokoro voice {}", voice_ids[0])
    } else {
        format!("Kokoro voices · {}", voice_ids.len())
    };

    let queue_ids = queue_download_plan(
        &app,
        &asset_root,
        &install_id,
        &plan,
        "voice",
        None,
        None,
        Some(display),
    )
    .await?;

    Ok(KokoroQueuedInstall {
        install_id,
        queue_ids,
    })
}

pub fn uninstall_model(
    asset_root: &Path,
    variant: KokoroModelVariant,
) -> Result<bool, KokoroError> {
    let mut removed = false;
    for filename in variant.candidate_model_filenames() {
        let path = asset_root.join("onnx").join(filename);
        if path.exists() {
            fs::remove_file(&path)?;
            removed = true;
        }
    }
    Ok(removed)
}

pub fn uninstall_voice(asset_root: &Path, voice_id: &str) -> Result<bool, KokoroError> {
    if !is_valid_voice_id(voice_id) {
        return Err(KokoroError::VoiceParse(format!(
            "Unsupported voice id: {}",
            voice_id
        )));
    }
    let path = asset_root.join("voices").join(format!("{voice_id}.bin"));
    if path.exists() {
        fs::remove_file(&path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
