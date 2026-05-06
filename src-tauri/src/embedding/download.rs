use super::*;
use crate::chat_manager::prompts;
use crate::storage_manager::settings::{internal_read_settings, settings_set_advanced};
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

fn apply_embedding_version_preference(
    advanced_settings: &mut serde_json::Value,
    version: &str,
) -> Result<(), String> {
    let Some(obj) = advanced_settings.as_object_mut() else {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Advanced settings payload is not an object",
        ));
    };

    obj.insert(
        "embeddingModelVersion".to_string(),
        serde_json::Value::String(version.to_string()),
    );

    Ok(())
}

fn persist_embedding_version_preference(app: &AppHandle, version: &str) -> Result<(), String> {
    let settings_json =
        internal_read_settings(app)?.ok_or_else(|| "Settings not found".to_string())?;
    let settings_value: serde_json::Value = serde_json::from_str(&settings_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse settings: {}", e),
        )
    })?;

    let mut advanced_settings = settings_value
        .get("advancedSettings")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    apply_embedding_version_preference(&mut advanced_settings, version)?;

    let advanced_json = serde_json::to_string(&advanced_settings).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize advanced settings: {}", e),
        )
    })?;

    settings_set_advanced(app.clone(), advanced_json)
}

fn delete_files(model_dir: &Path, files: &[&str]) -> Result<(), String> {
    for filename in files.iter() {
        let file_path = model_dir.join(filename);
        if file_path.exists() {
            fs::remove_file(&file_path).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to delete file {}: {}", filename, e),
                )
            })?;
        }
        let temp_path = file_path.with_extension("tmp");
        if temp_path.exists() {
            let _ = fs::remove_file(&temp_path);
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
    let groups: &[(&str, &[&str])] = &[
        ("v1", &MODEL_FILES_V1),
        ("v2", &MODEL_FILES_V2_LOCAL),
        ("v3", &MODEL_FILES_V3_LOCAL),
        ("v4", &MODEL_FILES_V4_LOCAL),
        ("companion-emotion", &COMPANION_EMOTION_MODEL_FILES_LOCAL),
        ("companion-ner", &COMPANION_NER_MODEL_FILES_LOCAL),
        ("companion-router", &COMPANION_ROUTER_MODEL_FILES_LOCAL),
    ];
    for (label, files) in groups {
        for filename in files.iter() {
            let path = model_dir.join(filename);
            log_info(
                app,
                component,
                format!(
                    "model file {} {}: {}",
                    label,
                    filename,
                    describe_path(&path)
                ),
            );
        }
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
    if let Some(parent) = temp_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create parent directory: {}", e),
            )
        })?;
    }
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

/// Generic download orchestration. Runs `plan` sequentially against `model_dir`, emitting progress
/// events. On failure or cancel, removes only `owned_files` from disk (so a failed companion
/// download doesn't wipe the embedding model).
async fn run_download_plan(
    app: &AppHandle,
    component: &str,
    plan: Vec<DownloadFileSpec>,
    owned_files: Vec<&'static str>,
) -> Result<(), String> {
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
            total_files: plan.len(),
            current_file_name: plan
                .first()
                .map(|item| item.progress_name.to_string())
                .unwrap_or_default(),
        };
        let _ = app.emit("embedding_download_progress", &state.progress);
    }

    let model_dir = embedding_model_dir(app)?;
    log_info(
        app,
        component,
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

    let state = DOWNLOAD_STATE.clone();

    for (file_index, file_spec) in plan.iter().enumerate() {
        let url = format!("{}/{}", file_spec.base_url, file_spec.remote_path);
        let dest_path = model_dir.join(file_spec.local_path);
        let display_file_name = file_spec.progress_name.to_string();

        {
            let mut state_lock = state.lock().await;
            state_lock.progress.status = format!("Downloading {}", display_file_name);
            state_lock.progress.current_file_index = file_index + 1;
            state_lock.progress.current_file_name = display_file_name.clone();
            let _ = app.emit("embedding_download_progress", &state_lock.progress);
        }

        log_info(
            app,
            component,
            format!(
                "download file {} of {}: {}",
                file_index + 1,
                plan.len(),
                file_spec.local_path
            ),
        );
        if let Err(e) = download_file(app, &url, &dest_path, state.clone()).await {
            log_error(
                app,
                component,
                format!("download failed file={} error={}", file_spec.local_path, e),
            );
            let _ = delete_files(&model_dir, &owned_files);
            let mut state_lock = state.lock().await;
            state_lock.is_downloading = false;
            state_lock.progress.status = "failed".to_string();
            let _ = app.emit("embedding_download_progress", &state_lock.progress);
            return Err(e);
        }
    }

    {
        let mut state_lock = state.lock().await;
        state_lock.is_downloading = false;
        state_lock.progress.status = "completed".to_string();
        let _ = app.emit("embedding_download_progress", &state_lock.progress);
    }

    Ok(())
}

pub async fn start_embedding_download(
    app: AppHandle,
    version: Option<String>,
) -> Result<(), String> {
    // v1, v2, and v3 are no longer offered as downloadable options. Existing
    // installs are still detected and used for inference, but new downloads
    // and upgrades always target the current model (v4).
    if let Some(requested) = version.as_deref() {
        let normalized = requested.to_ascii_lowercase();
        if matches!(normalized.as_str(), "v1" | "v2" | "v3") {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Embedding model {} is no longer downloadable. Use v4.",
                    normalized
                ),
            ));
        }
    }

    super::inference::clear_loaded_runtime_cache().await;

    let source_spec = download_source_spec(version.as_deref());
    let target_version = source_spec.target_version.clone();
    let plan = embedding_download_plan(version.as_deref());
    let owned_files = embedding_owned_files(&target_version);

    log_info(
        &app,
        "embedding_download",
        format!(
            "embedding download init requested={:?} source={} files={:?}",
            target_version,
            source_spec.source_label,
            plan.iter().map(|f| f.local_path).collect::<Vec<_>>()
        ),
    );

    persist_embedding_version_preference(&app, target_version.label())?;
    log_info(
        &app,
        "embedding_download",
        format!(
            "set preferred embedding model version to {}",
            target_version.label()
        ),
    );

    run_download_plan(&app, "embedding_download", plan, owned_files).await?;

    let model_dir = embedding_model_dir(&app)?;
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

#[cfg(test)]
mod tests {
    use super::apply_embedding_version_preference;

    #[test]
    fn sets_embedding_version_preference_in_advanced_settings() {
        let mut advanced = serde_json::json!({
            "embeddingMaxTokens": 2048
        });

        apply_embedding_version_preference(&mut advanced, "v4").expect("should update settings");

        assert_eq!(
            advanced.get("embeddingModelVersion"),
            Some(&serde_json::json!("v4"))
        );
        assert_eq!(
            advanced.get("embeddingMaxTokens"),
            Some(&serde_json::json!(2048))
        );
    }

    #[test]
    fn rejects_non_object_advanced_settings() {
        let mut advanced = serde_json::json!(null);

        let err = apply_embedding_version_preference(&mut advanced, "v4")
            .expect_err("non-object settings should fail");

        assert!(err.contains("Advanced settings payload is not an object"));
    }
}

pub async fn start_companion_download(app: AppHandle, kind: String) -> Result<(), String> {
    let kind = CompanionKind::from_str(&kind).ok_or_else(|| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unknown companion model kind: {}", kind),
        )
    })?;

    match kind {
        CompanionKind::Emotion => super::emotion::clear_loaded_runtime_cache().await,
        CompanionKind::Ner => super::ner::clear_loaded_runtime_cache().await,
        CompanionKind::Router => super::router::clear_loaded_runtime_cache().await,
    }

    let plan = companion_download_plan(kind);
    let owned_files: Vec<&'static str> = kind.local_files().to_vec();

    log_info(
        &app,
        "companion_download",
        format!(
            "companion download init kind={} files={:?}",
            kind.label(),
            plan.iter().map(|f| f.local_path).collect::<Vec<_>>()
        ),
    );

    run_download_plan(&app, "companion_download", plan, owned_files).await?;

    let model_dir = embedding_model_dir(&app)?;
    log_model_file_status(&app, "companion_download", &model_dir);

    Ok(())
}

pub async fn get_embedding_download_progress() -> Result<DownloadProgress, String> {
    let state = DOWNLOAD_STATE.lock().await;
    Ok(state.progress.clone())
}

/// Cancel an in-progress download. Signals the active `download_file` loop to
/// abort, which cleans up its own `.tmp` partial. Already-completed files from
/// the same download attempt are **left in place**; if the user wants them
/// gone they can run `delete_embedding_model_version` (or the companion
/// equivalent) explicitly. This separation prevents the historical bug where
/// cancelling one download would wipe unrelated installed models.
pub async fn cancel_embedding_download(app: AppHandle) -> Result<(), String> {
    {
        let mut state = DOWNLOAD_STATE.lock().await;
        if !state.is_downloading {
            return Ok(());
        }
        state.cancel_requested = true;
    }

    log_info(&app, "embedding_download", "cancel requested");

    // Give the active `download_file` loop a moment to notice the flag and
    // clean up its in-flight `.tmp` file.
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

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
    let mut files = MODEL_FILES_V1.to_vec();
    files.extend(MODEL_FILES_V2_LOCAL.iter().copied());
    files.extend(MODEL_FILES_V2_LOCAL_LEGACY.iter().copied());
    files.extend(MODEL_FILES_V3_LOCAL.iter().copied());
    files.extend(MODEL_FILES_V4_LOCAL.iter().copied());
    delete_files(&model_dir, &files)?;

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

    let files: Vec<&'static str> = match version_lower.as_str() {
        "v1" => MODEL_FILES_V1.to_vec(),
        "v2" => {
            let mut v = MODEL_FILES_V2_LOCAL.to_vec();
            v.extend(MODEL_FILES_V2_LOCAL_LEGACY.iter().copied());
            v
        }
        "v3" => MODEL_FILES_V3_LOCAL.to_vec(),
        "v4" => MODEL_FILES_V4_LOCAL.to_vec(),
        _ => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Unsupported embedding model version: {}", version),
            ));
        }
    };

    delete_files(&model_dir, &files)?;
    Ok(())
}

pub async fn delete_companion_model(app: AppHandle, kind: String) -> Result<(), String> {
    let kind = CompanionKind::from_str(&kind).ok_or_else(|| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unknown companion model kind: {}", kind),
        )
    })?;

    match kind {
        CompanionKind::Emotion => super::emotion::clear_loaded_runtime_cache().await,
        CompanionKind::Ner => super::ner::clear_loaded_runtime_cache().await,
        CompanionKind::Router => super::router::clear_loaded_runtime_cache().await,
    }

    let model_dir = embedding_model_dir(&app)?;
    log_info(
        &app,
        "companion_download",
        format!(
            "delete companion model {} in {}",
            kind.label(),
            model_dir.display()
        ),
    );
    let files: Vec<&'static str> = kind.local_files().to_vec();
    delete_files(&model_dir, &files)?;

    Ok(())
}
