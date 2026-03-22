use super::*;
use crate::chat_manager::prompts;
use crate::utils::{log_error, log_info, log_warn};
use futures_util::StreamExt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Emitter;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as TokioMutex;

pub async fn reset_download_state() {
    let mut state = DOWNLOAD_STATE.lock().await;
    state.is_downloading = false;
    state.cancel_requested = false;
    state.progress = DownloadProgress {
        downloaded: 0,
        total: 0,
        status: "idle".to_string(),
        current_file_index: 0,
        total_files: 0,
        current_file_name: String::new(),
    };
}

fn cleanup_partial_files(
    model_dir: &Path,
    version: Option<&EmbeddingModelVersion>,
) -> Result<(), String> {
    let files = match version {
        Some(EmbeddingModelVersion::V1) => MODEL_FILES_V1.to_vec(),
        Some(EmbeddingModelVersion::V2) => {
            let mut v = MODEL_FILES_V2_LOCAL.to_vec();
            v.extend(MODEL_FILES_V2_LOCAL_LEGACY.iter().copied());
            v
        }
        Some(EmbeddingModelVersion::V3) => MODEL_FILES_V3_LOCAL.to_vec(),
        None => {
            let mut all_files = MODEL_FILES_V1.to_vec();
            all_files.extend(MODEL_FILES_V2_LOCAL.iter().copied());
            all_files.extend(MODEL_FILES_V2_LOCAL_LEGACY.iter().copied());
            all_files.extend(MODEL_FILES_V3_LOCAL.iter().copied());
            all_files
        }
    };

    for filename in files.iter() {
        let file_path = model_dir.join(filename);
        if file_path.exists() {
            fs::remove_file(&file_path).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to delete partial file {}: {}", filename, e),
                )
            })?;
        }
    }
    Ok(())
}

fn describe_path(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(meta) => format!(
            "exists=true file={} dir={} size_bytes={}",
            meta.is_file(),
            meta.is_dir(),
            meta.len()
        ),
        Err(err) => format!("exists=false error={}", err),
    }
}

fn log_model_file_status(app: &AppHandle, component: &str, model_dir: &PathBuf) {
    for filename in MODEL_FILES_V1.iter() {
        let path = model_dir.join(filename);
        log_info(
            app,
            component,
            format!("model file v1 {}: {}", filename, describe_path(&path)),
        );
    }

    for filename in MODEL_FILES_V2_LOCAL.iter() {
        let path = model_dir.join(filename);
        log_info(
            app,
            component,
            format!("model file v2 {}: {}", filename, describe_path(&path)),
        );
    }
    for filename in MODEL_FILES_V3_LOCAL.iter() {
        let path = model_dir.join(filename);
        log_info(
            app,
            component,
            format!("model file v3 {}: {}", filename, describe_path(&path)),
        );
    }
}

async fn download_file(
    app: &AppHandle,
    url: &str,
    dest_path: &PathBuf,
    state: Arc<TokioMutex<DownloadState>>,
) -> Result<(), String> {
    log_info(
        app,
        "embedding_download",
        format!(
            "download start url={} dest={} temp={}",
            url,
            dest_path.display(),
            dest_path.with_extension("tmp").display()
        ),
    );
    let client = reqwest::Client::new();
    let response = client.get(url).send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to start download: {}", e),
        )
    })?;

    if !response.status().is_success() {
        log_error(
            app,
            "embedding_download",
            format!("download failed status={} url={}", response.status(), url),
        );
        return Err(format!(
            "Download failed with status: {}",
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);
    log_info(
        app,
        "embedding_download",
        format!("download response ok url={} total_size={}", url, total_size),
    );

    {
        let mut state_lock = state.lock().await;
        state_lock.progress.total += total_size;
        let _ = app.emit("embedding_download_progress", &state_lock.progress);
    }

    let temp_path = dest_path.with_extension("tmp");
    let mut file = tokio::fs::File::create(&temp_path).await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create file: {}", e),
        )
    })?;

    let mut stream = response.bytes_stream();
    let mut _downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk_result) = stream.next().await {
        {
            let state_lock = state.lock().await;
            if state_lock.cancel_requested {
                drop(file);
                let _ = tokio::fs::remove_file(&temp_path).await;
                let _ = app.emit("embedding_download_progress", &state_lock.progress);
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    "Download cancelled",
                ));
            }
        }

        let chunk = chunk_result.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Error reading chunk: {}", e),
            )
        })?;
        file.write_all(&chunk).await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Error writing to file: {}", e),
            )
        })?;

        _downloaded += chunk.len() as u64;

        {
            let mut state_lock = state.lock().await;
            state_lock.progress.downloaded += chunk.len() as u64;

            if last_emit.elapsed().as_millis() > 100 {
                let _ = app.emit("embedding_download_progress", &state_lock.progress);
                last_emit = std::time::Instant::now();
            }
        }
    }

    file.flush().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Error flushing file: {}", e),
        )
    })?;
    drop(file);

    tokio::fs::rename(&temp_path, dest_path)
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to rename file: {}", e),
            )
        })?;

    let file_status = describe_path(dest_path);
    log_info(
        app,
        "embedding_download",
        format!(
            "download complete dest={} {}",
            dest_path.display(),
            file_status
        ),
    );

    Ok(())
}

pub async fn start_embedding_download(
    app: AppHandle,
    version: Option<String>,
) -> Result<(), String> {
    super::inference::clear_loaded_runtime_cache().await;

    let source_spec = download_source_spec(version.as_deref());
    let target_version = source_spec.target_version;
    let source_label = source_spec.source_label;
    let base_url = source_spec.base_url;
    let remote_files = source_spec.remote_files.to_vec();
    let local_files = source_spec.local_files.to_vec();

    log_info(
        &app,
        "embedding_download",
        format!(
            "download init requested={:?} source={} base_url={} remote_files={:?} local_files={:?}",
            target_version, source_label, base_url, remote_files, local_files
        ),
    );

    {
        let mut state = DOWNLOAD_STATE.lock().await;
        if state.is_downloading {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Download already in progress",
            ));
        }
        state.is_downloading = true;
        state.cancel_requested = false;
        state.progress = DownloadProgress {
            downloaded: 0,
            total: 0,
            status: "downloading".to_string(),
            current_file_index: 1,
            total_files: remote_files.len(),
            current_file_name: local_files
                .first()
                .map(|s| s.to_string())
                .unwrap_or_default(),
        };
        let _ = app.emit("embedding_download_progress", &state.progress);
    }

    let model_dir = embedding_model_dir(&app)?;
    log_info(
        &app,
        "embedding_download",
        format!(
            "model_dir={} {}",
            model_dir.display(),
            describe_path(&model_dir)
        ),
    );
    fs::create_dir_all(&model_dir).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create model directory: {}", e),
        )
    })?;
    log_info(
        &app,
        "embedding_download",
        format!("model_dir ready {}", model_dir.display()),
    );

    let state = DOWNLOAD_STATE.clone();

    for (file_index, (remote_filename, local_filename)) in
        remote_files.iter().zip(local_files.iter()).enumerate()
    {
        let url = format!("{}/{}", base_url, remote_filename);
        let dest_path = model_dir.join(local_filename);
        let display_file_name = if source_label == "v3" {
            remote_filename.to_string()
        } else {
            local_filename.to_string()
        };

        {
            let mut state_lock = state.lock().await;
            state_lock.progress.status = format!("Downloading {}", display_file_name);
            state_lock.progress.current_file_index = file_index + 1;
            state_lock.progress.current_file_name = display_file_name.clone();
            let _ = app.emit("embedding_download_progress", &state_lock.progress);
        }

        log_info(
            &app,
            "embedding_download",
            format!(
                "download file {} of {}: {}",
                file_index + 1,
                remote_files.len(),
                local_filename
            ),
        );
        match download_file(&app, &url, &dest_path, state.clone()).await {
            Ok(_) => {}
            Err(e) => {
                log_error(
                    &app,
                    "embedding_download",
                    format!("download failed file={} error={}", local_filename, e),
                );
                if source_label == "v3" {
                    for filename in MODEL_FILES_V3_LOCAL.iter() {
                        let path = model_dir.join(filename);
                        if path.exists() {
                            let _ = fs::remove_file(path);
                        }
                    }
                } else {
                    let _ = cleanup_partial_files(&model_dir, Some(&target_version));
                }
                let mut state_lock = state.lock().await;
                state_lock.is_downloading = false;
                state_lock.progress.status = "failed".to_string();
                let _ = app.emit("embedding_download_progress", &state_lock.progress);
                return Err(e);
            }
        }
    }

    {
        let mut state_lock = state.lock().await;
        state_lock.is_downloading = false;
        state_lock.progress.status = "completed".to_string();
        let _ = app.emit("embedding_download_progress", &state_lock.progress);
    }

    log_model_file_status(&app, "embedding_download", &model_dir);

    if let Err(err) = prompts::ensure_dynamic_memory_templates(&app) {
        log_warn(
            &app,
            "embedding_download",
            format!("Failed to ensure dynamic memory prompts: {}", err),
        );
    }

    Ok(())
}

pub async fn get_embedding_download_progress() -> Result<DownloadProgress, String> {
    let state = DOWNLOAD_STATE.lock().await;
    Ok(state.progress.clone())
}

pub async fn cancel_embedding_download(app: AppHandle) -> Result<(), String> {
    super::inference::clear_loaded_runtime_cache().await;

    {
        let mut state = DOWNLOAD_STATE.lock().await;
        if !state.is_downloading {
            return Ok(());
        }
        state.cancel_requested = true;
    }

    log_info(&app, "embedding_download", "cancel requested");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let model_dir = embedding_model_dir(&app)?;
    cleanup_partial_files(&model_dir, None)?;

    {
        let mut state = DOWNLOAD_STATE.lock().await;
        state.is_downloading = false;
        state.cancel_requested = false;
        state.progress = DownloadProgress {
            downloaded: 0,
            total: 0,
            status: "cancelled".to_string(),
            current_file_index: 0,
            total_files: 0,
            current_file_name: String::new(),
        };
    }

    Ok(())
}

pub async fn delete_embedding_model(app: AppHandle) -> Result<(), String> {
    super::inference::clear_loaded_runtime_cache().await;
    reset_download_state().await;

    let model_dir = embedding_model_dir(&app)?;
    log_info(
        &app,
        "embedding_download",
        format!("delete embedding model files in {}", model_dir.display()),
    );
    cleanup_partial_files(&model_dir, None)?;

    Ok(())
}

pub async fn delete_embedding_model_version(app: AppHandle, version: String) -> Result<(), String> {
    super::inference::clear_loaded_runtime_cache().await;
    reset_download_state().await;

    let model_dir = embedding_model_dir(&app)?;
    layout::migrate_legacy_layout(&model_dir)?;
    let version_lower = version.to_lowercase();
    log_info(
        &app,
        "embedding_download",
        format!(
            "delete embedding model files for version={} in {}",
            version_lower,
            model_dir.display()
        ),
    );

    match version_lower.as_str() {
        "v1" => {
            for filename in MODEL_FILES_V1.iter() {
                let path = model_dir.join(filename);
                if path.exists() {
                    fs::remove_file(&path).map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Failed to delete {}: {}", path.display(), e),
                        )
                    })?;
                }
            }
        }
        "v2" => {
            let v2_data_path = model_dir.join("v2-model.onnx.data");
            if v2_data_path.exists() {
                let v2_model_path = model_dir.join("v2-model.onnx");
                if v2_model_path.exists() {
                    fs::remove_file(&v2_model_path).map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Failed to delete {}: {}", v2_model_path.display(), e),
                        )
                    })?;
                }
            }
            if v2_data_path.exists() {
                fs::remove_file(&v2_data_path).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to delete {}: {}", v2_data_path.display(), e),
                    )
                })?;
            }

            let v2_tokenizer = model_dir.join("v2-tokenizer.json");
            if v2_tokenizer.exists() {
                fs::remove_file(&v2_tokenizer).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to delete {}: {}", v2_tokenizer.display(), e),
                    )
                })?;
            }
        }
        "v3" => {
            for filename in MODEL_FILES_V3_LOCAL.iter() {
                let path = model_dir.join(filename);
                if path.exists() {
                    fs::remove_file(&path).map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Failed to delete {}: {}", path.display(), e),
                        )
                    })?;
                }
            }
        }
        _ => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Unsupported embedding model version: {}", version),
            ));
        }
    }

    Ok(())
}
