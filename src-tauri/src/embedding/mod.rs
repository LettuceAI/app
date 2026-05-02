use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex as TokioMutex;

mod benchmark;
mod download;
pub(crate) mod emotion;
mod inference;
mod layout;
pub(crate) mod ner;
mod ort_runtime;
pub(crate) mod router;
mod settings;
mod specs;
mod tests;
pub mod tokenizer;
mod util;

use specs::*;

const MAX_SEQ_LENGTH_V1: usize = 512;
const MAX_SEQ_LENGTH_MODERN: usize = 4096;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) const ORT_VERSION: &str = "1.22.0";
const EMBEDDING_DIM_LEGACY: usize = 512;
const EMBEDDING_DIM_V4: usize = 768;
const EMBEDDING_TEST_TIMEOUT_SECS: u64 = 90;
const EMBEDDING_BENCH_MAX_SEQ_LENGTH: usize = 1024;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Default)]
pub enum EmbeddingModelVersion {
    V1,
    V2,
    V3,
    #[default]
    V4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EmbeddingSourceVersion {
    V1,
    V3,
    V4,
}

impl EmbeddingSourceVersion {
    fn as_str(self) -> &'static str {
        match self {
            EmbeddingSourceVersion::V1 => "v1",
            EmbeddingSourceVersion::V3 => "v3",
            EmbeddingSourceVersion::V4 => "v4",
        }
    }
}

#[derive(Clone, Debug)]
struct ActiveEmbeddingConfig {
    source: EmbeddingSourceVersion,
    version_label: &'static str,
    model_path: PathBuf,
    tokenizer_path: PathBuf,
    max_seq_length: usize,
    embedding_dimensions: usize,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingModelInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub source_version: Option<String>,
    pub selected_source_version: Option<String>,
    pub available_versions: Vec<String>,
    pub max_tokens: u32,
    pub companion_emotion_installed: bool,
    pub companion_ner_installed: bool,
    pub companion_router_installed: bool,
    pub install_bundle_complete: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub status: String,
    pub current_file_index: usize,
    pub total_files: usize,
    pub current_file_name: String,
}

struct DownloadState {
    progress: DownloadProgress,
    cancel_requested: bool,
    is_downloading: bool,
}

lazy_static::lazy_static! {
    static ref DOWNLOAD_STATE: Arc<TokioMutex<DownloadState>> = Arc::new(TokioMutex::new(DownloadState {
        progress: DownloadProgress {
            downloaded: 0,
            total: 0,
            status: "idle".to_string(),
            current_file_index: 0,
            total_files: 0,
            current_file_name: String::new(),
        },
        cancel_requested: false,
        is_downloading: false,
    }));
}

static ORT_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn embedding_model_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let lettuce_dir = crate::utils::lettuce_dir(app)?;
    Ok(lettuce_dir.join("models").join("embedding"))
}

fn resolve_selected_source_version(
    app: &AppHandle,
    _has_v1: bool,
    _has_v2: bool,
    has_v3: bool,
    has_v4: bool,
) -> Option<EmbeddingSourceVersion> {
    let preferred = settings::read_embedding_preferences(app).preferred_source_version;

    match preferred.as_deref() {
        Some("v4") if has_v4 => Some(EmbeddingSourceVersion::V4),
        Some("v3") if has_v3 => Some(EmbeddingSourceVersion::V3),
        _ => {
            if has_v4 {
                Some(EmbeddingSourceVersion::V4)
            } else if has_v3 {
                Some(EmbeddingSourceVersion::V3)
            } else {
                None
            }
        }
    }
}

fn resolve_model_paths(
    model_dir: &Path,
    source: EmbeddingSourceVersion,
) -> (PathBuf, PathBuf, usize, &'static str) {
    match source {
        EmbeddingSourceVersion::V1 => (
            model_dir.join("lettuce-emb-512d-kd-v1.onnx"),
            model_dir.join("tokenizer.json"),
            MAX_SEQ_LENGTH_V1,
            "v1",
        ),
        EmbeddingSourceVersion::V3 => (
            model_dir.join("v3-model.int8.onnx"),
            model_dir.join("v3-tokenizer.json"),
            MAX_SEQ_LENGTH_MODERN,
            "v3",
        ),
        EmbeddingSourceVersion::V4 => (
            model_dir.join("v4-model.int8.onnx"),
            model_dir.join("v4-tokenizer.json"),
            MAX_SEQ_LENGTH_MODERN,
            "v4",
        ),
    }
}

fn resolve_target_embedding_dimensions(
    source: EmbeddingSourceVersion,
    preferred_dimensions: Option<usize>,
) -> usize {
    match source {
        EmbeddingSourceVersion::V4 => match preferred_dimensions.unwrap_or(EMBEDDING_DIM_V4) {
            64 | 128 | 256 | 512 | 768 => preferred_dimensions.unwrap_or(EMBEDDING_DIM_V4),
            _ => EMBEDDING_DIM_V4,
        },
        _ => EMBEDDING_DIM_LEGACY,
    }
}

fn resolve_runtime_model(app: &AppHandle) -> Result<ActiveEmbeddingConfig, String> {
    let model_dir = embedding_model_dir(app)?;
    layout::migrate_legacy_layout(&model_dir)?;
    let installed = layout::detect_installed_sources(&model_dir);
    let source = resolve_selected_source_version(
        app,
        installed.has_v1,
        installed.has_v2,
        installed.has_v3,
        installed.has_v4,
    )
    .ok_or_else(|| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            "Model files not found. Please download the model first.",
        )
    })?;

    let (model_path, tokenizer_path, mut max_seq_length, label) =
        resolve_model_paths(&model_dir, source);
    if source != EmbeddingSourceVersion::V1 {
        let settings_max_tokens = settings::read_embedding_preferences(app).max_tokens;
        let resolved_max_tokens = settings_max_tokens.unwrap_or(MAX_SEQ_LENGTH_MODERN);
        max_seq_length = resolved_max_tokens.clamp(512, MAX_SEQ_LENGTH_MODERN);
    }

    let prefs = settings::read_embedding_preferences(app);
    let embedding_dimensions =
        resolve_target_embedding_dimensions(source, prefs.embedding_dimensions);

    Ok(ActiveEmbeddingConfig {
        source,
        version_label: label,
        model_path,
        tokenizer_path,
        max_seq_length,
        embedding_dimensions,
    })
}

pub(crate) fn resolve_active_embedding_signature(
    app: &AppHandle,
) -> Result<(String, usize), String> {
    let active = resolve_runtime_model(app)?;
    Ok((active.source.as_str().to_string(), active.embedding_dimensions))
}

#[tauri::command]
pub fn check_embedding_model(app: AppHandle) -> Result<bool, String> {
    let model_dir = embedding_model_dir(&app)?;
    layout::migrate_legacy_layout(&model_dir)?;
    let installed = layout::detect_installed_sources(&model_dir);
    Ok(installed.has_v3 || installed.has_v4)
}

pub fn detect_model_version(app: &AppHandle) -> Result<Option<EmbeddingModelVersion>, String> {
    let model_dir = embedding_model_dir(app)?;
    layout::migrate_legacy_layout(&model_dir)?;
    let installed = layout::detect_installed_sources(&model_dir);
    if installed.has_v4 {
        return Ok(Some(EmbeddingModelVersion::V4));
    }
    if installed.has_v3 {
        return Ok(Some(EmbeddingModelVersion::V3));
    }
    if installed.has_v2 {
        return Ok(Some(EmbeddingModelVersion::V2));
    }
    if installed.has_v1 {
        return Ok(Some(EmbeddingModelVersion::V1));
    }
    Ok(None)
}

impl EmbeddingModelVersion {
    fn label(&self) -> &'static str {
        match self {
            EmbeddingModelVersion::V1 => "v1",
            EmbeddingModelVersion::V2 => "v2",
            EmbeddingModelVersion::V3 => "v3",
            EmbeddingModelVersion::V4 => "v4",
        }
    }
}

#[tauri::command]
pub fn get_embedding_model_info(app: AppHandle) -> Result<EmbeddingModelInfo, String> {
    let model_dir = embedding_model_dir(&app)?;
    layout::migrate_legacy_layout(&model_dir)?;
    let installed = layout::detect_installed_sources(&model_dir);

    let mut available_versions = Vec::new();
    if installed.has_v1 {
        available_versions.push("v1".to_string());
    }
    if installed.has_v2 {
        available_versions.push("v2".to_string());
    }
    if installed.has_v3 {
        available_versions.push("v3".to_string());
    }
    if installed.has_v4 {
        available_versions.push("v4".to_string());
    }

    let selected_source = resolve_selected_source_version(
        &app,
        installed.has_v1,
        installed.has_v2,
        installed.has_v3,
        installed.has_v4,
    );
    let source_version = selected_source.map(|v| v.as_str().to_string());
    let max_tokens = match selected_source {
        Some(EmbeddingSourceVersion::V1) => MAX_SEQ_LENGTH_V1 as u32,
        Some(_) => MAX_SEQ_LENGTH_MODERN as u32,
        None => 0,
    };

    let detected = detect_model_version(&app)?;
    let companions_complete = installed.has_companion_emotion
        && installed.has_companion_ner
        && installed.has_companion_router;

    Ok(match detected {
        Some(version) => EmbeddingModelInfo {
            installed: true,
            version: Some(version.label().to_string()),
            source_version: source_version.clone(),
            selected_source_version: source_version,
            available_versions,
            max_tokens,
            companion_emotion_installed: installed.has_companion_emotion,
            companion_ner_installed: installed.has_companion_ner,
            companion_router_installed: installed.has_companion_router,
            install_bundle_complete: companions_complete,
        },
        None => EmbeddingModelInfo {
            installed: false,
            version: None,
            source_version: None,
            selected_source_version: None,
            available_versions: vec![],
            max_tokens: 0,
            companion_emotion_installed: installed.has_companion_emotion,
            companion_ner_installed: installed.has_companion_ner,
            companion_router_installed: installed.has_companion_router,
            install_bundle_complete: false,
        },
    })
}

pub use download::reset_download_state;

#[tauri::command]
pub async fn start_embedding_download(
    app: AppHandle,
    version: Option<String>,
) -> Result<(), String> {
    download::start_embedding_download(app, version).await
}

#[tauri::command]
pub async fn get_embedding_download_progress() -> Result<DownloadProgress, String> {
    download::get_embedding_download_progress().await
}

#[tauri::command]
pub async fn cancel_embedding_download(app: AppHandle) -> Result<(), String> {
    download::cancel_embedding_download(app).await
}

#[tauri::command]
pub async fn delete_embedding_model(app: AppHandle) -> Result<(), String> {
    download::delete_embedding_model(app).await
}

#[tauri::command]
pub async fn delete_embedding_model_version(app: AppHandle, version: String) -> Result<(), String> {
    download::delete_embedding_model_version(app, version).await
}

#[tauri::command]
pub async fn start_companion_download(app: AppHandle, kind: String) -> Result<(), String> {
    download::start_companion_download(app, kind).await
}

#[tauri::command]
pub async fn delete_companion_model(app: AppHandle, kind: String) -> Result<(), String> {
    download::delete_companion_model(app, kind).await
}

#[tauri::command]
pub async fn compute_embedding(app: AppHandle, text: String) -> Result<Vec<f32>, String> {
    inference::compute_embedding(app, text).await
}

#[tauri::command]
pub async fn initialize_embedding_model(app: AppHandle) -> Result<(), String> {
    inference::initialize_embedding_model(app).await
}

#[tauri::command]
pub async fn clear_embedding_runtime_cache() -> Result<(), String> {
    inference::clear_loaded_runtime_cache().await;
    emotion::clear_loaded_runtime_cache().await;
    ner::clear_loaded_runtime_cache().await;
    router::clear_loaded_runtime_cache().await;
    Ok(())
}

#[tauri::command]
pub async fn run_embedding_dev_benchmark(
    app: AppHandle,
) -> Result<benchmark::DevBenchmarkResult, String> {
    benchmark::run_embedding_dev_benchmark(app).await
}

#[tauri::command]
pub async fn run_embedding_test(app: AppHandle) -> Result<tests::TestResult, String> {
    tests::run_embedding_test(app).await
}

#[tauri::command]
pub async fn compare_custom_texts(
    app: AppHandle,
    text_a: String,
    text_b: String,
) -> Result<f32, String> {
    tests::compare_custom_texts(app, text_a, text_b).await
}
