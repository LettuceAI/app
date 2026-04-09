use super::*;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::mtmd::{MtmdContext, MtmdContextParams};
use serde_json::json;
use std::ffi::{c_void, CString};
use std::path::Path;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::Emitter;

#[derive(Clone)]
pub(super) struct LoadedEngine {
    pub(super) backend: Arc<LlamaBackend>,
    pub(super) model: Arc<LlamaModel>,
    pub(super) backend_path_used: Option<String>,
    pub(super) actual_gpu_layers_used: Option<u32>,
    pub(super) gpu_load_fallback_activated: bool,
    pub(super) gpu_load_fallback_reason: Option<String>,
    pub(super) smart_gpu_layer_fallback_activated: bool,
    pub(super) compiled_gpu_backends: Vec<String>,
    pub(super) supports_gpu_offload: bool,
    pub(super) mtmd_ctx: Option<Arc<MtmdContext>>,
}

pub(super) struct LlamaState {
    pub(super) backend: Option<Arc<LlamaBackend>>,
    pub(super) model_path: Option<String>,
    pub(super) model_params_key: Option<String>,
    pub(super) model: Option<Arc<LlamaModel>>,
    pub(super) backend_path_used: Option<String>,
    pub(super) actual_gpu_layers_used: Option<u32>,
    pub(super) gpu_load_fallback_activated: bool,
    pub(super) gpu_load_fallback_reason: Option<String>,
    pub(super) smart_gpu_layer_fallback_activated: bool,
    pub(super) compiled_gpu_backends: Vec<String>,
    pub(super) supports_gpu_offload: bool,
    pub(super) mtmd_ctx: Option<Arc<MtmdContext>>,
    pub(super) mmproj_path: Option<String>,
    pub(super) kqv_fallback_toast_shown: bool,
}

const LLAMA_MODEL_LOAD_PROGRESS_EVENT: &str = "llama-model-load-progress";
const MODEL_LOAD_STAGE_GPU_OFFLOAD: u8 = 0;
const MODEL_LOAD_STAGE_CPU: u8 = 1;
const MODEL_LOAD_STAGE_CPU_FALLBACK: u8 = 2;
const MODEL_LOAD_STAGE_FINALIZING: u8 = 3;
const MODEL_LOAD_STATUS_LOADING: u8 = 0;
const MODEL_LOAD_STATUS_RETRYING: u8 = 1;
const MODEL_LOAD_STATUS_LOADED: u8 = 2;
const MODEL_LOAD_STATUS_FAILED: u8 = 3;
const MODEL_LOAD_PROGRESS_CAP: f32 = 0.9;
const MODEL_LOAD_FINALIZING_PROGRESS: f32 = 0.95;

struct ModelLoadProgressContext {
    app: AppHandle,
    request_id: Option<String>,
    model_path: String,
    backend_path: String,
    stage: u8,
    last_percent: AtomicU8,
}

fn model_display_name(model_path: &str) -> String {
    Path::new(model_path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .unwrap_or_else(|| model_path.to_string())
}

fn emit_model_load_progress(
    app: &AppHandle,
    request_id: Option<&str>,
    model_path: &str,
    backend_path: &str,
    stage: u8,
    status: u8,
    progress: f32,
) {
    let clamped = progress.clamp(0.0, 1.0);
    let percent = (clamped * 100.0).round().clamp(0.0, 100.0) as u8;
    let _ = app.emit(
        LLAMA_MODEL_LOAD_PROGRESS_EVENT,
        json!({
            "requestId": request_id,
            "modelPath": model_path,
            "modelName": model_display_name(model_path),
            "backendPath": backend_path,
            "stage": stage,
            "status": status,
            "progress": clamped,
            "percent": percent,
        }),
    );
}

fn stage_progress(progress: f32) -> f32 {
    progress.clamp(0.0, 1.0) * MODEL_LOAD_PROGRESS_CAP
}

unsafe extern "C" fn model_load_progress_callback(progress: f32, user_data: *mut c_void) -> bool {
    if user_data.is_null() {
        return true;
    }

    let ctx = unsafe { &*(user_data as *const ModelLoadProgressContext) };
    let clamped = stage_progress(progress);
    let percent = (clamped * 100.0).round().clamp(0.0, 100.0) as u8;
    let last = ctx.last_percent.load(Ordering::Relaxed);
    if percent <= last && percent < 100 {
        return true;
    }

    ctx.last_percent.store(percent, Ordering::Relaxed);
    emit_model_load_progress(
        &ctx.app,
        ctx.request_id.as_deref(),
        &ctx.model_path,
        &ctx.backend_path,
        ctx.stage,
        MODEL_LOAD_STATUS_LOADING,
        clamped,
    );
    true
}

fn model_load_stage_for_backend(
    backend_path: Option<&str>,
    gpu_load_fallback_activated: bool,
) -> u8 {
    match backend_path {
        Some("gpu_offload") => MODEL_LOAD_STAGE_GPU_OFFLOAD,
        Some("cpu") if gpu_load_fallback_activated => MODEL_LOAD_STAGE_CPU_FALLBACK,
        Some("cpu") => MODEL_LOAD_STAGE_CPU,
        _ => MODEL_LOAD_STAGE_FINALIZING,
    }
}

pub(super) fn emit_model_load_finalizing(
    app: &AppHandle,
    request_id: Option<&str>,
    model_path: &str,
    backend_path: Option<&str>,
    _gpu_load_fallback_activated: bool,
) {
    emit_model_load_progress(
        app,
        request_id,
        model_path,
        backend_path.unwrap_or("unknown"),
        MODEL_LOAD_STAGE_FINALIZING,
        MODEL_LOAD_STATUS_LOADING,
        MODEL_LOAD_FINALIZING_PROGRESS,
    );
}

pub(super) fn emit_model_load_complete(
    app: &AppHandle,
    request_id: Option<&str>,
    model_path: &str,
    backend_path: Option<&str>,
    gpu_load_fallback_activated: bool,
) {
    emit_model_load_progress(
        app,
        request_id,
        model_path,
        backend_path.unwrap_or("unknown"),
        model_load_stage_for_backend(backend_path, gpu_load_fallback_activated),
        MODEL_LOAD_STATUS_LOADED,
        1.0,
    );
}

pub(super) fn emit_model_load_failed(
    app: &AppHandle,
    request_id: Option<&str>,
    model_path: &str,
    backend_path: Option<&str>,
    gpu_load_fallback_activated: bool,
) {
    emit_model_load_progress(
        app,
        request_id,
        model_path,
        backend_path.unwrap_or("unknown"),
        model_load_stage_for_backend(backend_path, gpu_load_fallback_activated),
        MODEL_LOAD_STATUS_FAILED,
        0.0,
    );
}

fn load_model_with_progress(
    app: Option<&AppHandle>,
    request_id: Option<&str>,
    model_path: &str,
    n_gpu_layers: Option<u32>,
    backend_path: &str,
    stage: u8,
) -> Result<LlamaModel, String> {
    let cstr = CString::new(model_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid llama model path: {e}"),
        )
    })?;

    let mut params = unsafe { llama_cpp_sys_2::llama_model_default_params() };
    if let Some(n_gpu_layers) = n_gpu_layers {
        params.n_gpu_layers = i32::try_from(n_gpu_layers).unwrap_or(i32::MAX);
    }

    let progress_ctx = app.map(|app| {
        Box::new(ModelLoadProgressContext {
            app: app.clone(),
            request_id: request_id.map(ToOwned::to_owned),
            model_path: model_path.to_string(),
            backend_path: backend_path.to_string(),
            stage,
            last_percent: AtomicU8::new(0),
        })
    });

    if let Some(ctx) = progress_ctx.as_ref() {
        emit_model_load_progress(
            &ctx.app,
            ctx.request_id.as_deref(),
            &ctx.model_path,
            &ctx.backend_path,
            ctx.stage,
            MODEL_LOAD_STATUS_LOADING,
            0.0,
        );
        params.progress_callback = Some(model_load_progress_callback);
        params.progress_callback_user_data = ctx.as_ref() as *const _ as *mut c_void;
    }

    let raw_model = unsafe { llama_cpp_sys_2::llama_load_model_from_file(cstr.as_ptr(), params) };
    let model_ptr = NonNull::new(raw_model).ok_or_else(|| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            "Failed to load llama model: null reference from llama.cpp",
        )
    })?;

    // SAFETY: `LlamaModel` is a transparent wrapper around `NonNull<llama_model>`.
    let model = unsafe {
        std::mem::transmute::<NonNull<llama_cpp_sys_2::llama_model>, LlamaModel>(model_ptr)
    };
    Ok(model)
}

fn compiled_gpu_backends() -> Vec<&'static str> {
    let mut out = Vec::new();
    if cfg!(feature = "llama-gpu-cuda") || cfg!(feature = "llama-gpu-cuda-no-vmm") {
        out.push("cuda");
    }
    if cfg!(feature = "llama-gpu-rocm") {
        out.push("rocm");
    }
    if cfg!(feature = "llama-gpu-vulkan") {
        out.push("vulkan");
    }
    if cfg!(feature = "llama-gpu-metal") {
        out.push("metal");
    }
    out
}

pub(super) fn using_rocm_backend() -> bool {
    cfg!(feature = "llama-gpu-rocm")
}

static ENGINE: OnceLock<Mutex<LlamaState>> = OnceLock::new();
static SHARED_BACKEND: OnceLock<Arc<LlamaBackend>> = OnceLock::new();

pub(crate) fn shared_backend() -> Result<Arc<LlamaBackend>, String> {
    if let Some(backend) = SHARED_BACKEND.get() {
        return Ok(backend.clone());
    }

    let backend = Arc::new(LlamaBackend::init().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to initialize llama backend: {e}"),
        )
    })?);

    let _ = SHARED_BACKEND.set(backend.clone());
    SHARED_BACKEND.get().cloned().ok_or_else(|| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            "Failed to cache shared llama backend",
        )
    })
}

pub(super) fn load_engine(
    app: Option<&AppHandle>,
    request_id: Option<&str>,
    model_path: &str,
    requested_gpu_layers: Option<u32>,
    auto_gpu_layer_candidates: Option<&[u32]>,
    strict_mode: bool,
    mmproj_path: Option<&str>,
) -> Result<LoadedEngine, String> {
    let engine = ENGINE.get_or_init(|| {
        Mutex::new(LlamaState {
            backend: None,
            model_path: None,
            model_params_key: None,
            model: None,
            backend_path_used: None,
            actual_gpu_layers_used: None,
            gpu_load_fallback_activated: false,
            gpu_load_fallback_reason: None,
            smart_gpu_layer_fallback_activated: false,
            compiled_gpu_backends: Vec::new(),
            supports_gpu_offload: false,
            mtmd_ctx: None,
            mmproj_path: None,
            kqv_fallback_toast_shown: false,
        })
    });

    let mut guard = engine
        .lock()
        .map_err(|_| "llama.cpp engine lock poisoned".to_string())?;

    if guard.backend.is_none() {
        guard.backend = Some(shared_backend()?);
    }

    let supports_gpu = guard
        .backend
        .as_ref()
        .ok_or_else(|| "llama.cpp backend unavailable".to_string())?
        .supports_gpu_offload();
    let gpu_backends = compiled_gpu_backends();
    let gpu_backend_label = if gpu_backends.is_empty() {
        "none".to_string()
    } else {
        gpu_backends.join(",")
    };
    guard.compiled_gpu_backends = gpu_backends.iter().map(|v| (*v).to_string()).collect();
    guard.supports_gpu_offload = supports_gpu;
    if let Some(app) = app {
        log_info(
            app,
            "llama_cpp",
            format!(
                "llama.cpp backend initialized: compiled_gpu_backends={} supports_gpu_offload={}",
                gpu_backend_label, supports_gpu
            ),
        );
    }
    if let (Some(app), Some(requested)) = (app, requested_gpu_layers) {
        if requested > 0 && !supports_gpu {
            if strict_mode {
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "Strict mode is enabled and llamaGpuLayers={} requires GPU offload, but this build has no active GPU backend.",
                        requested
                    ),
                ));
            }
            log_warn(
                app,
                "llama_cpp",
                format!(
                    "Requested llamaGpuLayers={} but this build has no active GPU offload; using CPU layers only.",
                    requested
                ),
            );
        }
    }
    let requested_gpu_layers_key = if auto_gpu_layer_candidates.is_some() {
        "smart:auto".to_string()
    } else {
        requested_gpu_layers
            .map(|v| v.to_string())
            .unwrap_or_else(|| "auto".to_string())
    };
    let model_params_key = format!(
        "requested_gpu_layers={requested_gpu_layers_key};strict_mode={}",
        strict_mode
    );
    let should_reload = guard.model.is_none()
        || guard.model_path.as_deref() != Some(model_path)
        || guard.model_params_key.as_deref() != Some(&model_params_key);
    if !should_reload {
        if let Some(app) = app {
            log_info(
                app,
                "llama_cpp",
                format!(
                    "Reusing loaded llama.cpp model: model_path={} actual_gpu_layers_used={:?} backend_path={}",
                    model_path,
                    guard.actual_gpu_layers_used,
                    guard.backend_path_used.as_deref().unwrap_or("unknown"),
                ),
            );
        }
    }
    if should_reload {
        let mut backend_path_used = "cpu".to_string();
        let mut actual_gpu_layers_used = None;
        let mut gpu_load_fallback_activated = false;
        let mut gpu_load_fallback_reason = None;
        let mut smart_gpu_layer_fallback_activated = false;
        guard.kqv_fallback_toast_shown = false;

        let model = if supports_gpu && requested_gpu_layers != Some(0) {
            if let Some(candidates) = auto_gpu_layer_candidates.filter(|value| !value.is_empty()) {
                let mut gpu_attempt_error: Option<String> = None;
                let mut attempted_gpu_candidate = false;
                let mut resolved_model: Option<Arc<LlamaModel>> = None;

                for (index, candidate) in candidates.iter().copied().enumerate() {
                    if candidate == 0 {
                        break;
                    }

                    attempted_gpu_candidate = true;
                    if index > 0 {
                        smart_gpu_layer_fallback_activated = true;
                        if let Some(app) = app {
                            emit_model_load_progress(
                                app,
                                request_id,
                                model_path,
                                "gpu_offload",
                                MODEL_LOAD_STAGE_GPU_OFFLOAD,
                                MODEL_LOAD_STATUS_RETRYING,
                                0.0,
                            );
                        }
                    }

                    match load_model_with_progress(
                        app,
                        request_id,
                        model_path,
                        Some(candidate),
                        "gpu_offload",
                        MODEL_LOAD_STAGE_GPU_OFFLOAD,
                    ) {
                        Ok(model) => {
                            backend_path_used = "gpu_offload".to_string();
                            actual_gpu_layers_used = Some(candidate);
                            if let Some(app) = app {
                                if smart_gpu_layer_fallback_activated {
                                    log_warn(
                                        app,
                                        "llama_cpp",
                                        format!(
                                            "Smart GPU offload backed off to {} layers after earlier load failures",
                                            candidate
                                        ),
                                    );
                                } else {
                                    log_info(
                                        app,
                                        "llama_cpp",
                                        format!(
                                            "Loaded model with smart GPU offload at {} layers",
                                            candidate
                                        ),
                                    );
                                }
                            }
                            resolved_model = Some(Arc::new(model));
                            break;
                        }
                        Err(err) => {
                            gpu_attempt_error = Some(err.to_string());
                            if let Some(app) = app {
                                log_warn(
                                    app,
                                    "llama_cpp",
                                    format!(
                                        "Smart GPU offload attempt failed at {} layers: {}",
                                        candidate, err
                                    ),
                                );
                            }
                        }
                    }
                }

                if let Some(model) = resolved_model {
                    model
                } else {
                    if strict_mode {
                        return Err(crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            "Strict mode is enabled, so llama.cpp will not fall back after smart GPU offload failure.",
                        ));
                    }
                    if attempted_gpu_candidate {
                        gpu_load_fallback_activated = true;
                        gpu_load_fallback_reason = gpu_attempt_error.clone();
                        if let Some(app) = app {
                            emit_model_load_progress(
                                app,
                                request_id,
                                model_path,
                                "cpu",
                                MODEL_LOAD_STAGE_CPU_FALLBACK,
                                MODEL_LOAD_STATUS_RETRYING,
                                0.0,
                            );
                            log_warn(
                                app,
                                "llama_cpp",
                                format!(
                                    "Smart GPU offload exhausted GPU layer candidates {:?}, falling back to CPU: {}",
                                    candidates,
                                    gpu_attempt_error.as_deref().unwrap_or("unknown GPU load error")
                                ),
                            );
                            let _ = app.emit(
                                "app://toast",
                                json!({
                                    "variant": "warning",
                                    "title": "GPU fallback",
                                    "description": "Model did not fit in GPU memory. Switched to CPU automatically."
                                }),
                            );
                        }
                    }

                    actual_gpu_layers_used = Some(0);
                    Arc::new(
                        load_model_with_progress(
                            app,
                            request_id,
                            model_path,
                            Some(0),
                            "cpu",
                            if attempted_gpu_candidate {
                                MODEL_LOAD_STAGE_CPU_FALLBACK
                            } else {
                                MODEL_LOAD_STAGE_CPU
                            },
                        )
                        .map_err(|err| {
                            if let Some(app) = app {
                                emit_model_load_progress(
                                    app,
                                    request_id,
                                    model_path,
                                    "cpu",
                                    if attempted_gpu_candidate {
                                        MODEL_LOAD_STAGE_CPU_FALLBACK
                                    } else {
                                        MODEL_LOAD_STAGE_CPU
                                    },
                                    MODEL_LOAD_STATUS_FAILED,
                                    0.0,
                                );
                            }
                            err
                        })?,
                    )
                }
            } else {
                match load_model_with_progress(
                    app,
                    request_id,
                    model_path,
                    requested_gpu_layers,
                    "gpu_offload",
                    MODEL_LOAD_STAGE_GPU_OFFLOAD,
                ) {
                    Ok(model) => {
                        backend_path_used = "gpu_offload".to_string();
                        actual_gpu_layers_used = requested_gpu_layers;
                        if let Some(app) = app {
                            let mode = requested_gpu_layers
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "llama-default".to_string());
                            log_info(
                                app,
                                "llama_cpp",
                                format!("Loaded model with GPU mode {}", mode),
                            );
                        }
                        Arc::new(model)
                    }
                    Err(err) => {
                        if strict_mode {
                            if let Some(app) = app {
                                emit_model_load_progress(
                                    app,
                                    request_id,
                                    model_path,
                                    "gpu_offload",
                                    MODEL_LOAD_STAGE_GPU_OFFLOAD,
                                    MODEL_LOAD_STATUS_FAILED,
                                    0.0,
                                );
                                log_warn(
                                    app,
                                    "llama_cpp",
                                    format!(
                                        "GPU model load failed with strict mode enabled; refusing CPU fallback: {}",
                                        err
                                    ),
                                );
                            }
                            return Err(crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!(
                                    "Strict mode is enabled, so llama.cpp will not fall back to CPU after GPU load failure: {}",
                                    err
                                ),
                            ));
                        }
                        gpu_load_fallback_activated = true;
                        gpu_load_fallback_reason = Some(err.to_string());
                        if let Some(app) = app {
                            emit_model_load_progress(
                                app,
                                request_id,
                                model_path,
                                "cpu",
                                MODEL_LOAD_STAGE_CPU_FALLBACK,
                                MODEL_LOAD_STATUS_RETRYING,
                                0.0,
                            );
                            log_warn(
                                app,
                                "llama_cpp",
                                format!("GPU model load failed, falling back to CPU: {}", err),
                            );
                            let _ = app.emit(
                                "app://toast",
                                json!({
                                    "variant": "warning",
                                    "title": "GPU fallback",
                                    "description": "Model did not fit in GPU memory. Switched to CPU automatically."
                                }),
                            );
                        }
                        actual_gpu_layers_used = Some(0);
                        Arc::new(
                            load_model_with_progress(
                                app,
                                request_id,
                                model_path,
                                Some(0),
                                "cpu",
                                MODEL_LOAD_STAGE_CPU_FALLBACK,
                            )
                            .map_err(|err| {
                                if let Some(app) = app {
                                    emit_model_load_progress(
                                        app,
                                        request_id,
                                        model_path,
                                        "cpu",
                                        MODEL_LOAD_STAGE_CPU_FALLBACK,
                                        MODEL_LOAD_STATUS_FAILED,
                                        0.0,
                                    );
                                }
                                err
                            })?,
                        )
                    }
                }
            }
        } else {
            actual_gpu_layers_used = Some(0);
            Arc::new(
                load_model_with_progress(
                    app,
                    request_id,
                    model_path,
                    Some(0),
                    "cpu",
                    MODEL_LOAD_STAGE_CPU,
                )
                .map_err(|err| {
                    if let Some(app) = app {
                        emit_model_load_progress(
                            app,
                            request_id,
                            model_path,
                            "cpu",
                            MODEL_LOAD_STAGE_CPU,
                            MODEL_LOAD_STATUS_FAILED,
                            0.0,
                        );
                    }
                    err
                })?,
            )
        };

        guard.model = Some(model);
        guard.model_path = Some(model_path.to_string());
        guard.model_params_key = Some(model_params_key);
        guard.backend_path_used = Some(backend_path_used);
        guard.actual_gpu_layers_used = actual_gpu_layers_used;
        guard.gpu_load_fallback_activated = gpu_load_fallback_activated;
        guard.gpu_load_fallback_reason = gpu_load_fallback_reason;
        guard.smart_gpu_layer_fallback_activated = smart_gpu_layer_fallback_activated;
    }

    let mmproj_changed = should_reload
        || guard.mmproj_path.as_deref() != mmproj_path
        || (mmproj_path.is_some() && guard.mtmd_ctx.is_none());
    if mmproj_changed {
        guard.mtmd_ctx = None;
        guard.mmproj_path = None;

        if let Some(mmproj_path) = mmproj_path {
            if !Path::new(mmproj_path).exists() {
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("mmproj file not found: {}", mmproj_path),
                ));
            }

            let model = guard
                .model
                .as_ref()
                .ok_or_else(|| "llama.cpp model unavailable for mtmd init".to_string())?;
            let mtmd = MtmdContext::init_from_file(
                mmproj_path,
                model.as_ref(),
                &MtmdContextParams::default(),
            )
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "Failed to initialize llama.cpp mtmd context from {}: {}",
                        mmproj_path, e
                    ),
                )
            })?;

            if let Some(app) = app {
                log_info(
                    app,
                    "llama_cpp",
                    format!(
                        "mtmd loaded: mmproj_path={} vision={} audio={}",
                        mmproj_path,
                        mtmd.support_vision(),
                        mtmd.support_audio()
                    ),
                );
            }

            guard.mtmd_ctx = Some(Arc::new(mtmd));
            guard.mmproj_path = Some(mmproj_path.to_string());
        }
    }

    Ok(LoadedEngine {
        backend: guard
            .backend
            .clone()
            .ok_or_else(|| "llama.cpp backend unavailable".to_string())?,
        model: guard
            .model
            .clone()
            .ok_or_else(|| "llama.cpp model unavailable".to_string())?,
        backend_path_used: guard.backend_path_used.clone(),
        actual_gpu_layers_used: guard.actual_gpu_layers_used,
        gpu_load_fallback_activated: guard.gpu_load_fallback_activated,
        gpu_load_fallback_reason: guard.gpu_load_fallback_reason.clone(),
        smart_gpu_layer_fallback_activated: guard.smart_gpu_layer_fallback_activated,
        compiled_gpu_backends: guard.compiled_gpu_backends.clone(),
        supports_gpu_offload: guard.supports_gpu_offload,
        mtmd_ctx: guard.mtmd_ctx.clone(),
    })
}

pub(crate) fn unload_engine(app: &AppHandle) -> Result<(), String> {
    let engine = ENGINE.get_or_init(|| {
        Mutex::new(LlamaState {
            backend: None,
            model_path: None,
            model_params_key: None,
            model: None,
            backend_path_used: None,
            actual_gpu_layers_used: None,
            gpu_load_fallback_activated: false,
            gpu_load_fallback_reason: None,
            smart_gpu_layer_fallback_activated: false,
            compiled_gpu_backends: Vec::new(),
            supports_gpu_offload: false,
            mtmd_ctx: None,
            mmproj_path: None,
            kqv_fallback_toast_shown: false,
        })
    });

    let mut guard = engine
        .lock()
        .map_err(|_| "llama.cpp engine lock poisoned".to_string())?;

    if guard.model.is_some() {
        guard.model = None;
        guard.model_path = None;
        guard.model_params_key = None;
        guard.backend_path_used = None;
        guard.actual_gpu_layers_used = None;
        guard.gpu_load_fallback_activated = false;
        guard.gpu_load_fallback_reason = None;
        guard.smart_gpu_layer_fallback_activated = false;
        guard.mtmd_ctx = None;
        guard.mmproj_path = None;
        guard.kqv_fallback_toast_shown = false;
        log_info(app, "llama_cpp", "unloaded llama.cpp model");
    }

    Ok(())
}

pub(crate) fn consume_kqv_fallback_toast(
    _app: &AppHandle,
    model_path: &str,
) -> Result<bool, String> {
    let engine = ENGINE.get_or_init(|| {
        Mutex::new(LlamaState {
            backend: None,
            model_path: None,
            model_params_key: None,
            model: None,
            backend_path_used: None,
            actual_gpu_layers_used: None,
            gpu_load_fallback_activated: false,
            gpu_load_fallback_reason: None,
            smart_gpu_layer_fallback_activated: false,
            compiled_gpu_backends: Vec::new(),
            supports_gpu_offload: false,
            mtmd_ctx: None,
            mmproj_path: None,
            kqv_fallback_toast_shown: false,
        })
    });

    let mut guard = engine
        .lock()
        .map_err(|_| "llama.cpp engine lock poisoned".to_string())?;

    if guard.model_path.as_deref() != Some(model_path) {
        return Ok(true);
    }

    if guard.kqv_fallback_toast_shown {
        return Ok(false);
    }

    guard.kqv_fallback_toast_shown = true;
    Ok(true)
}
