use super::*;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::params::LlamaModelParams;
use std::sync::{Mutex, OnceLock};

pub(super) struct LlamaState {
    pub(super) backend: Option<LlamaBackend>,
    pub(super) model_path: Option<String>,
    pub(super) model_params_key: Option<String>,
    pub(super) model: Option<LlamaModel>,
    pub(super) backend_path_used: Option<String>,
    pub(super) gpu_load_fallback_activated: bool,
    pub(super) compiled_gpu_backends: Vec<String>,
    pub(super) supports_gpu_offload: bool,
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

pub(super) fn load_engine(
    app: Option<&AppHandle>,
    model_path: &str,
    requested_gpu_layers: Option<u32>,
) -> Result<std::sync::MutexGuard<'static, LlamaState>, String> {
    let engine = ENGINE.get_or_init(|| {
        Mutex::new(LlamaState {
            backend: None,
            model_path: None,
            model_params_key: None,
            model: None,
            backend_path_used: None,
            gpu_load_fallback_activated: false,
            compiled_gpu_backends: Vec::new(),
            supports_gpu_offload: false,
        })
    });

    let mut guard = engine
        .lock()
        .map_err(|_| "llama.cpp engine lock poisoned".to_string())?;

    if guard.backend.is_none() {
        guard.backend = Some(LlamaBackend::init().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to initialize llama backend: {e}"),
            )
        })?);
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
    let backend = guard
        .backend
        .as_ref()
        .ok_or_else(|| "llama.cpp backend unavailable".to_string())?;
    if let (Some(app), Some(requested)) = (app, requested_gpu_layers) {
        if requested > 0 && !supports_gpu {
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
    let requested_gpu_layers_key = requested_gpu_layers
        .map(|v| v.to_string())
        .unwrap_or_else(|| "auto".to_string());
    let model_params_key = format!("requested_gpu_layers={requested_gpu_layers_key}");
    let should_reload = guard.model.is_none()
        || guard.model_path.as_deref() != Some(model_path)
        || guard.model_params_key.as_deref() != Some(&model_params_key);
    if should_reload {
        let cpu_params = LlamaModelParams::default().with_n_gpu_layers(0);
        let mut backend_path_used = "cpu".to_string();
        let mut gpu_load_fallback_activated = false;

        let model = if supports_gpu && requested_gpu_layers != Some(0) {
            let gpu_params = if let Some(explicit_layers) = requested_gpu_layers {
                LlamaModelParams::default().with_n_gpu_layers(explicit_layers)
            } else {
                // Let llama.cpp choose the default GPU offload policy/layers.
                LlamaModelParams::default()
            };

            match LlamaModel::load_from_file(backend, model_path, &gpu_params) {
                Ok(model) => {
                    backend_path_used = "gpu_offload".to_string();
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
                    model
                }
                Err(err) => {
                    gpu_load_fallback_activated = true;
                    if let Some(app) = app {
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
                    LlamaModel::load_from_file(backend, model_path, &cpu_params).map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Failed to load llama model: {e}"),
                        )
                    })?
                }
            }
        } else {
            LlamaModel::load_from_file(backend, model_path, &cpu_params).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to load llama model: {e}"),
                )
            })?
        };

        guard.model = Some(model);
        guard.model_path = Some(model_path.to_string());
        guard.model_params_key = Some(model_params_key);
        guard.backend_path_used = Some(backend_path_used);
        guard.gpu_load_fallback_activated = gpu_load_fallback_activated;
    }

    Ok(guard)
}

pub(crate) fn unload_engine(app: &AppHandle) -> Result<(), String> {
    let engine = ENGINE.get_or_init(|| {
        Mutex::new(LlamaState {
            backend: None,
            model_path: None,
            model_params_key: None,
            model: None,
            backend_path_used: None,
            gpu_load_fallback_activated: false,
            compiled_gpu_backends: Vec::new(),
            supports_gpu_offload: false,
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
        guard.gpu_load_fallback_activated = false;
        log_info(app, "llama_cpp", "unloaded llama.cpp model");
    }

    Ok(())
}
