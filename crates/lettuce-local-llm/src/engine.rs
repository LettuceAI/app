//! The loaded llama.cpp model and its sidecars (multimodal projector, MTP
//! draft model).
//!
//! A load reuses the model already in memory when the path and every load
//! parameter match, otherwise it drops the old model first (freeing VRAM) and
//! loads the new one: with smart offload it walks the GPU layer candidates
//! from the largest down, then falls back to the CPU unless strict mode
//! forbids it; with a fixed layer count it tries that once before the same
//! fallback. The projector and draft model reload whenever the model did or
//! their own settings changed.

use std::ffi::{CString, c_void};
use std::path::Path;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::mtmd::{MtmdContext, MtmdContextParams, measure_memory_usage};
use llama_cpp_2::{LlamaBackendDeviceType, list_llama_ggml_backend_devices};
use llama_cpp_sys_2::{
    GGML_BACKEND_DEVICE_TYPE_ACCEL, GGML_BACKEND_DEVICE_TYPE_GPU, GGML_BACKEND_DEVICE_TYPE_IGPU,
    ggml_backend_dev_count, ggml_backend_dev_get, ggml_backend_dev_type,
};

use crate::llama::shared_backend;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LlamaEngineError {
    #[error("{0}")]
    Backend(String),
    #[error("llama.cpp engine lock poisoned")]
    LockPoisoned,
    #[error("{0}")]
    Load(String),
    #[error("{0}")]
    StrictMode(String),
    #[error("{0}")]
    Sidecar(String),
    #[error("{0}")]
    NativeFit(String),
}

impl From<crate::llama::LlamaRuntimeError> for LlamaEngineError {
    fn from(error: crate::llama::LlamaRuntimeError) -> Self {
        Self::Backend(error.to_string())
    }
}

/// Where the model's weights ended up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendPath {
    GpuOffload,
    Cpu,
}

impl BackendPath {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuOffload => "gpu_offload",
            Self::Cpu => "cpu",
        }
    }
}

/// The model-load stage the frontend shows (legacy numeric values).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ModelLoadStage {
    GpuOffload = 0,
    Cpu = 1,
    CpuFallback = 2,
    Finalizing = 3,
}

/// The model-load status the frontend shows (legacy numeric values).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ModelLoadStatus {
    Loading = 0,
    Retrying = 1,
    Loaded = 2,
    Failed = 3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GpuLoadProgress {
    pub label: String,
    pub percent: u8,
}

/// One `llama-model-load-progress` update.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelLoadProgress {
    pub request_id: Option<String>,
    pub model_path: String,
    pub model_name: String,
    pub backend_path: String,
    pub stage: ModelLoadStage,
    pub status: ModelLoadStatus,
    pub progress: f32,
    pub percent: u8,
    pub gpus: Option<Vec<GpuLoadProgress>>,
}

/// Receives load progress and the one-time "switched to CPU" notice.
pub trait EngineObserver: Send + Sync {
    fn model_load_progress(&self, progress: ModelLoadProgress);

    /// The model did not fit in GPU memory and was loaded on the CPU.
    fn gpu_fallback(&self);
}

const MODEL_LOAD_PROGRESS_CAP: f32 = 0.9;
const MODEL_LOAD_FINALIZING_PROGRESS: f32 = 0.95;

#[derive(Clone, Debug, Default)]
pub struct LlamaGpuConfig {
    pub multi_gpu_enabled: bool,
    pub device_ids: Vec<usize>,
    pub device_labels: Vec<String>,
    pub tensor_split: Vec<f32>,
    pub main_gpu: Option<i32>,
    pub distribution_mode: Option<String>,
    pub total_layer_count: Option<u32>,
}

/// Parameters llama.cpp's own fitter chose (only used behind its gate).
pub struct NativeFitPlan {
    pub model_params: Pin<Box<LlamaModelParams>>,
    pub n_ctx: u32,
    pub n_gpu_layers: u32,
    pub tensor_split: Vec<f32>,
}

impl std::fmt::Debug for NativeFitPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeFitPlan")
            .field("n_ctx", &self.n_ctx)
            .field("n_gpu_layers", &self.n_gpu_layers)
            .field("tensor_split", &self.tensor_split)
            .finish_non_exhaustive()
    }
}

/// The per-device margins llama.cpp's fitter keeps free, grown by what the
/// multimodal projector will take on each selected device.
pub fn measure_mmproj_fit_margins(
    mmproj_path: Option<&str>,
    device_ids: &[usize],
) -> Result<Vec<usize>, LlamaEngineError> {
    let default_margin = LlamaModelParams::default_fit_margin();
    let mut margins = vec![0; llama_cpp_2::max_devices().max(1)];
    let devices = list_llama_ggml_backend_devices();
    let selected: Vec<_> = devices
        .iter()
        .filter(|device| {
            matches!(
                device.device_type,
                LlamaBackendDeviceType::Gpu | LlamaBackendDeviceType::Accelerator
            ) && (device_ids.is_empty() || device_ids.contains(&device.index))
        })
        .collect();
    for margin in margins.iter_mut().take(selected.len()) {
        *margin = default_margin;
    }
    let Some(mmproj_path) = mmproj_path else {
        return Ok(margins);
    };
    let usage =
        measure_memory_usage(mmproj_path, &MtmdContextParams::default()).map_err(|error| {
            LlamaEngineError::NativeFit(format!("Failed to measure MTMD memory usage: {error}"))
        })?;
    let mut assigned_sidecar_bytes = 0usize;
    for (position, device) in selected.iter().enumerate() {
        let measured = usage
            .iter()
            .filter(|entry| !entry.host && entry.device_name == device.name)
            .map(|entry| entry.bytes)
            .sum();
        if let Some(margin) = margins.get_mut(position) {
            *margin = margin.checked_add(measured).ok_or_else(|| {
                LlamaEngineError::NativeFit("MTMD fit margin overflowed usize".into())
            })?;
            assigned_sidecar_bytes =
                assigned_sidecar_bytes
                    .checked_add(measured)
                    .ok_or_else(|| {
                        LlamaEngineError::NativeFit("MTMD measured memory overflowed usize".into())
                    })?;
        }
    }
    let measured_accelerator_bytes: usize = usage
        .iter()
        .filter(|entry| !entry.host)
        .map(|entry| entry.bytes)
        .sum();
    if assigned_sidecar_bytes != measured_accelerator_bytes {
        return Err(LlamaEngineError::NativeFit(
            "MTMD memory measurement did not match the selected llama.cpp devices".into(),
        ));
    }
    Ok(margins)
}

/// Runs llama.cpp's own parameter fitter.
pub fn fit_model_params(
    model_path: &str,
    device_ids: &[usize],
    mut context_params: LlamaContextParams,
    margins: &[usize],
    n_ctx_min: u32,
    load_mtp: bool,
) -> Result<NativeFitPlan, LlamaEngineError> {
    let path = CString::new(model_path).map_err(|error| {
        LlamaEngineError::NativeFit(format!(
            "Invalid llama model path for native fitting: {error}"
        ))
    })?;
    let mut model_params = LlamaModelParams::default().with_load_mtp(load_mtp);
    if !device_ids.is_empty() {
        model_params = model_params.with_devices(device_ids).map_err(|error| {
            LlamaEngineError::NativeFit(format!(
                "Failed to select devices for native fitting: {error}"
            ))
        })?;
    }
    let mut model_params = Box::pin(model_params);
    let mut margins = margins.to_vec();
    margins.resize(llama_cpp_2::max_devices().max(1), 0);
    let result = model_params
        .as_mut()
        .fit_params(
            &path,
            &mut context_params,
            &mut margins,
            n_ctx_min,
            llama_cpp_sys_2::GGML_LOG_LEVEL_INFO,
        )
        .map_err(|error| {
            LlamaEngineError::NativeFit(format!(
                "llama.cpp native parameter fitting failed: {error}"
            ))
        })?;
    let n_gpu_layers = u32::try_from(model_params.n_gpu_layers().max(0)).unwrap_or(0);
    let tensor_split = model_params.tensor_split().to_vec();
    Ok(NativeFitPlan {
        model_params,
        n_ctx: result.n_ctx,
        n_gpu_layers,
        tensor_split,
    })
}

fn compute_gpu_progress_ranges(
    labels: &[String],
    tensor_split: &[f32],
    n_gpu_layers: Option<u32>,
    total_layer_count: Option<u32>,
) -> Vec<(String, f32, f32)> {
    let device_count = labels.len();
    if device_count < 2 || tensor_split.len() < device_count {
        return Vec::new();
    }
    let split_sum: f32 = tensor_split[..device_count].iter().sum();
    if split_sum <= 0.0 {
        return Vec::new();
    }
    let Some(total) = total_layer_count.filter(|value| *value > 0) else {
        return Vec::new();
    };
    let n_all = total.saturating_add(1) as f32;
    let act = n_gpu_layers
        .map(|value| (value as f32).min(n_all))
        .unwrap_or(n_all);
    if act <= 0.0 {
        return Vec::new();
    }
    let start_frac = (n_all - act) / n_all;
    let gpu_frac = act / n_all;
    let mut ranges = Vec::with_capacity(device_count);
    let mut cumulative = 0.0f32;
    for (index, label) in labels.iter().enumerate() {
        let start = start_frac + gpu_frac * (cumulative / split_sum);
        cumulative += tensor_split[index].max(0.0);
        let end = start_frac + gpu_frac * (cumulative / split_sum);
        ranges.push((label.clone(), start, end.max(start)));
    }
    ranges
}

fn gpu_progress_payload(
    gpu_ranges: &[(String, f32, f32)],
    raw_progress: f32,
) -> Option<Vec<GpuLoadProgress>> {
    if gpu_ranges.is_empty() {
        return None;
    }
    Some(
        gpu_ranges
            .iter()
            .map(|(label, start, end)| {
                let span = (end - start).max(f32::EPSILON);
                let device_progress = ((raw_progress - start) / span).clamp(0.0, 1.0);
                GpuLoadProgress {
                    label: label.clone(),
                    percent: (device_progress * 100.0).round().clamp(0.0, 100.0) as u8,
                }
            })
            .collect(),
    )
}

fn model_display_name(model_path: &str) -> String {
    Path::new(model_path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| model_path.to_string())
}

#[expect(
    clippy::too_many_arguments,
    reason = "the legacy progress event fields"
)]
fn emit_model_load_progress(
    observer: &dyn EngineObserver,
    request_id: Option<&str>,
    model_path: &str,
    backend_path: &str,
    stage: ModelLoadStage,
    status: ModelLoadStatus,
    progress: f32,
    gpus: Option<Vec<GpuLoadProgress>>,
) {
    let clamped = progress.clamp(0.0, 1.0);
    observer.model_load_progress(ModelLoadProgress {
        request_id: request_id.map(ToOwned::to_owned),
        model_path: model_path.to_string(),
        model_name: model_display_name(model_path),
        backend_path: backend_path.to_string(),
        stage,
        status,
        progress: clamped,
        percent: (clamped * 100.0).round().clamp(0.0, 100.0) as u8,
        gpus,
    });
}

fn stage_progress(progress: f32) -> f32 {
    progress.clamp(0.0, 1.0) * MODEL_LOAD_PROGRESS_CAP
}

struct ModelLoadProgressContext<'a> {
    observer: &'a dyn EngineObserver,
    request_id: Option<String>,
    model_path: String,
    backend_path: String,
    stage: ModelLoadStage,
    last_percent: AtomicU8,
    gpu_ranges: Vec<(String, f32, f32)>,
}

unsafe extern "C" fn model_load_progress_callback(progress: f32, user_data: *mut c_void) -> bool {
    if user_data.is_null() {
        return true;
    }
    let context = unsafe { &*user_data.cast::<ModelLoadProgressContext<'_>>() };
    let clamped = stage_progress(progress);
    let percent = (clamped * 100.0).round().clamp(0.0, 100.0) as u8;
    let last = context.last_percent.load(Ordering::Relaxed);
    if percent <= last && percent < 100 {
        return true;
    }
    context.last_percent.store(percent, Ordering::Relaxed);
    emit_model_load_progress(
        context.observer,
        context.request_id.as_deref(),
        &context.model_path,
        &context.backend_path,
        context.stage,
        ModelLoadStatus::Loading,
        clamped,
        gpu_progress_payload(&context.gpu_ranges, progress.clamp(0.0, 1.0)),
    );
    true
}

fn model_load_stage_for_backend(
    backend_path: Option<BackendPath>,
    gpu_load_fallback_activated: bool,
) -> ModelLoadStage {
    match backend_path {
        Some(BackendPath::GpuOffload) => ModelLoadStage::GpuOffload,
        Some(BackendPath::Cpu) if gpu_load_fallback_activated => ModelLoadStage::CpuFallback,
        Some(BackendPath::Cpu) => ModelLoadStage::Cpu,
        None => ModelLoadStage::Finalizing,
    }
}

fn backend_label(backend_path: Option<BackendPath>) -> &'static str {
    backend_path.map_or("unknown", BackendPath::as_str)
}

pub fn emit_model_load_finalizing(
    observer: &dyn EngineObserver,
    request_id: Option<&str>,
    model_path: &str,
    backend_path: Option<BackendPath>,
) {
    emit_model_load_progress(
        observer,
        request_id,
        model_path,
        backend_label(backend_path),
        ModelLoadStage::Finalizing,
        ModelLoadStatus::Loading,
        MODEL_LOAD_FINALIZING_PROGRESS,
        None,
    );
}

pub fn emit_model_load_complete(
    observer: &dyn EngineObserver,
    request_id: Option<&str>,
    model_path: &str,
    backend_path: Option<BackendPath>,
    gpu_load_fallback_activated: bool,
) {
    emit_model_load_progress(
        observer,
        request_id,
        model_path,
        backend_label(backend_path),
        model_load_stage_for_backend(backend_path, gpu_load_fallback_activated),
        ModelLoadStatus::Loaded,
        1.0,
        None,
    );
}

pub fn emit_model_load_failed(
    observer: &dyn EngineObserver,
    request_id: Option<&str>,
    model_path: &str,
    backend_path: Option<BackendPath>,
    gpu_load_fallback_activated: bool,
) {
    emit_model_load_progress(
        observer,
        request_id,
        model_path,
        backend_label(backend_path),
        model_load_stage_for_backend(backend_path, gpu_load_fallback_activated),
        ModelLoadStatus::Failed,
        0.0,
        None,
    );
}

fn resolve_selected_gpu_device(
    device_id: usize,
    allow_integrated: bool,
) -> Result<llama_cpp_sys_2::ggml_backend_dev_t, LlamaEngineError> {
    let count = unsafe { ggml_backend_dev_count() };
    if device_id >= count {
        return Err(LlamaEngineError::Load(format!(
            "Selected GPU device index {device_id} is not available."
        )));
    }
    let device = unsafe { ggml_backend_dev_get(device_id) };
    if device.is_null() {
        return Err(LlamaEngineError::Load(format!(
            "Selected GPU device index {device_id} resolved to null."
        )));
    }
    let device_type = unsafe { ggml_backend_dev_type(device) };
    let is_gpu_like = device_type == GGML_BACKEND_DEVICE_TYPE_GPU
        || device_type == GGML_BACKEND_DEVICE_TYPE_ACCEL
        || (allow_integrated && device_type == GGML_BACKEND_DEVICE_TYPE_IGPU);
    if !is_gpu_like {
        return Err(LlamaEngineError::Load(format!(
            "Selected device index {device_id} is not a discrete GPU device."
        )));
    }
    Ok(device)
}

#[expect(clippy::too_many_arguments, reason = "the legacy model load inputs")]
fn load_model_with_progress(
    observer: Option<&dyn EngineObserver>,
    request_id: Option<&str>,
    model_path: &str,
    n_gpu_layers: Option<u32>,
    gpu_config: &LlamaGpuConfig,
    backend_path: BackendPath,
    stage: ModelLoadStage,
    fitted_params: Option<&LlamaModelParams>,
    load_mtp: bool,
) -> Result<LlamaModel, LlamaEngineError> {
    let mut params = fitted_params.map_or_else(
        || unsafe { llama_cpp_sys_2::llama_model_default_params() },
        |params| *params.as_raw(),
    );
    if fitted_params.is_none()
        && let Some(n_gpu_layers) = n_gpu_layers
    {
        params.n_gpu_layers = i32::try_from(n_gpu_layers).unwrap_or(i32::MAX);
    }
    params.load_mtp = load_mtp;
    let mut selected_devices = Vec::new();
    let tensor_split_storage: Vec<f32> = fitted_params.map_or_else(
        || gpu_config.tensor_split.clone(),
        |params| params.tensor_split().to_vec(),
    );
    if fitted_params.is_none() && gpu_config.multi_gpu_enabled {
        if gpu_config.device_ids.len() < 2 {
            return Err(LlamaEngineError::Load(
                "Multi-GPU mode requires at least two selected GPU devices.".into(),
            ));
        }
        for device_id in &gpu_config.device_ids {
            selected_devices.push(resolve_selected_gpu_device(*device_id, false)?);
        }
        selected_devices.push(std::ptr::null_mut());
        params.devices = selected_devices.as_mut_ptr();
        params.split_mode = llama_cpp_sys_2::LLAMA_SPLIT_MODE_LAYER;
        if !tensor_split_storage.is_empty() {
            params.tensor_split = tensor_split_storage.as_ptr();
        }
        if let Some(main_gpu) = gpu_config.main_gpu {
            params.main_gpu = main_gpu;
        }
    } else if fitted_params.is_none() && gpu_config.device_ids.len() == 1 {
        selected_devices.push(resolve_selected_gpu_device(gpu_config.device_ids[0], true)?);
        selected_devices.push(std::ptr::null_mut());
        params.devices = selected_devices.as_mut_ptr();
        params.main_gpu = 0;
    }

    let path = CString::new(model_path)
        .map_err(|error| LlamaEngineError::Load(format!("Invalid llama model path: {error}")))?;
    let gpu_ranges = if gpu_config.multi_gpu_enabled || !tensor_split_storage.is_empty() {
        compute_gpu_progress_ranges(
            &gpu_config.device_labels,
            &tensor_split_storage,
            n_gpu_layers,
            gpu_config.total_layer_count,
        )
    } else {
        Vec::new()
    };
    let progress_context = observer.map(|observer| {
        Box::new(ModelLoadProgressContext {
            observer,
            request_id: request_id.map(ToOwned::to_owned),
            model_path: model_path.to_string(),
            backend_path: backend_path.as_str().to_string(),
            stage,
            last_percent: AtomicU8::new(0),
            gpu_ranges,
        })
    });
    if let Some(context) = progress_context.as_ref() {
        emit_model_load_progress(
            context.observer,
            context.request_id.as_deref(),
            &context.model_path,
            &context.backend_path,
            context.stage,
            ModelLoadStatus::Loading,
            0.0,
            gpu_progress_payload(&context.gpu_ranges, 0.0),
        );
        params.progress_callback = Some(model_load_progress_callback);
        params.progress_callback_user_data =
            std::ptr::from_ref::<ModelLoadProgressContext<'_>>(context.as_ref())
                .cast_mut()
                .cast::<c_void>();
    }

    let raw_model = unsafe { llama_cpp_sys_2::llama_load_model_from_file(path.as_ptr(), params) };
    let model_ptr = NonNull::new(raw_model).ok_or_else(|| {
        LlamaEngineError::Load("Failed to load llama model: null reference from llama.cpp".into())
    })?;
    drop(progress_context);
    Ok(unsafe {
        std::mem::transmute::<NonNull<llama_cpp_sys_2::llama_model>, LlamaModel>(model_ptr)
    })
}

/// The GPU backends this build was compiled with.
#[must_use]
pub fn compiled_gpu_backends() -> Vec<&'static str> {
    let mut out = Vec::new();
    if cfg!(feature = "cuda") || cfg!(feature = "cuda-no-vmm") {
        out.push("cuda");
    }
    if cfg!(feature = "rocm") {
        out.push("rocm");
    }
    if cfg!(feature = "vulkan") {
        out.push("vulkan");
    }
    if cfg!(feature = "metal") {
        out.push("metal");
    }
    out
}

#[must_use]
pub const fn using_rocm_backend() -> bool {
    cfg!(feature = "rocm")
}

/// A loaded model with everything a generation needs.
#[derive(Clone)]
pub struct LoadedEngine {
    pub model_reloaded: bool,
    pub backend: Arc<LlamaBackend>,
    pub model: Arc<LlamaModel>,
    pub backend_path_used: Option<BackendPath>,
    pub actual_gpu_layers_used: Option<u32>,
    pub gpu_load_fallback_activated: bool,
    pub gpu_load_fallback_reason: Option<String>,
    pub smart_gpu_layer_fallback_activated: bool,
    pub compiled_gpu_backends: Vec<String>,
    pub supports_gpu_offload: bool,
    pub mtmd_ctx: Option<Arc<MtmdContext>>,
    pub mtp_model: Option<Arc<LlamaModel>>,
}

impl std::fmt::Debug for LoadedEngine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoadedEngine")
            .field("model_reloaded", &self.model_reloaded)
            .field("backend_path_used", &self.backend_path_used)
            .field("actual_gpu_layers_used", &self.actual_gpu_layers_used)
            .field(
                "gpu_load_fallback_activated",
                &self.gpu_load_fallback_activated,
            )
            .field("mtmd", &self.mtmd_ctx.is_some())
            .field("mtp", &self.mtp_model.is_some())
            .finish_non_exhaustive()
    }
}

/// Everything one load needs.
#[derive(Debug)]
pub struct EngineLoadRequest<'a> {
    pub request_id: Option<&'a str>,
    pub model_path: &'a str,
    pub requested_gpu_layers: Option<u32>,
    pub auto_gpu_layer_candidates: Option<&'a [u32]>,
    pub native_fit_plan: Option<&'a NativeFitPlan>,
    pub gpu_config: LlamaGpuConfig,
    pub strict_mode: bool,
    pub mmproj_path: Option<&'a str>,
    pub load_bundled_mtp: bool,
    pub mtp_model_path: Option<&'a str>,
    pub mtp_drafter_on_gpu: bool,
    pub mtp_gpu_fallback_allowed: bool,
    pub mtp_gpu_device_id: Option<usize>,
}

#[derive(Default)]
struct LlamaState {
    backend: Option<Arc<LlamaBackend>>,
    model_path: Option<String>,
    model_params_key: Option<String>,
    model: Option<Arc<LlamaModel>>,
    backend_path_used: Option<BackendPath>,
    actual_gpu_layers_used: Option<u32>,
    gpu_load_fallback_activated: bool,
    gpu_load_fallback_reason: Option<String>,
    smart_gpu_layer_fallback_activated: bool,
    compiled_gpu_backends: Vec<String>,
    supports_gpu_offload: bool,
    mtmd_ctx: Option<Arc<MtmdContext>>,
    mmproj_path: Option<String>,
    mtp_model: Option<Arc<LlamaModel>>,
    mtp_model_path: Option<String>,
    mtp_model_config_key: Option<String>,
    kqv_fallback_toast_shown: bool,
}

impl LlamaState {
    fn drop_model(&mut self) {
        self.model = None;
        self.model_path = None;
        self.model_params_key = None;
        self.backend_path_used = None;
        self.actual_gpu_layers_used = None;
        self.gpu_load_fallback_activated = false;
        self.gpu_load_fallback_reason = None;
        self.smart_gpu_layer_fallback_activated = false;
        self.mtmd_ctx = None;
        self.mmproj_path = None;
        self.mtp_model = None;
        self.mtp_model_path = None;
        self.mtp_model_config_key = None;
    }
}

fn model_params_key(request: &EngineLoadRequest<'_>) -> String {
    let requested_gpu_layers_key = if let Some(candidates) = request.auto_gpu_layer_candidates {
        let candidate_key = candidates
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!("smart:{candidate_key}")
    } else {
        request
            .requested_gpu_layers
            .map_or_else(|| "auto".to_string(), |value| value.to_string())
    };
    let native_fit_key = request.native_fit_plan.map_or_else(
        || "none".to_string(),
        |plan| {
            format!(
                "ctx={},layers={},split={}",
                plan.n_ctx,
                plan.n_gpu_layers,
                plan.tensor_split
                    .iter()
                    .map(|value| format!("{value:.4}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        },
    );
    let gpu_config = &request.gpu_config;
    format!(
        "requested_gpu_layers={requested_gpu_layers_key};native_fit={native_fit_key};load_mtp={};strict_mode={};multi_gpu={};devices={};distribution={};tensor_split={};main_gpu={}",
        request.load_bundled_mtp,
        request.strict_mode,
        gpu_config.multi_gpu_enabled,
        gpu_config
            .device_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        gpu_config
            .distribution_mode
            .as_deref()
            .unwrap_or("balanced"),
        gpu_config
            .tensor_split
            .iter()
            .map(|value| format!("{value:.4}"))
            .collect::<Vec<_>>()
            .join(","),
        gpu_config
            .main_gpu
            .map_or_else(|| "auto".to_string(), |value| value.to_string()),
    )
}

/// The llama.cpp model slot. One model (plus its sidecars) is loaded at a
/// time; loads and unloads are serialized by the slot's lock.
#[derive(Default)]
pub struct LlamaEngine {
    state: Mutex<LlamaState>,
}

impl std::fmt::Debug for LlamaEngine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LlamaEngine")
            .finish_non_exhaustive()
    }
}

impl LlamaEngine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads (or reuses) the requested model. `discard_hot_context` runs
    /// before a different model replaces the loaded one, so a cached context
    /// holding the old model is released first.
    #[expect(
        clippy::too_many_lines,
        reason = "the legacy load decision ladder, kept in one place"
    )]
    pub fn load(
        &self,
        observer: Option<&dyn EngineObserver>,
        request: &EngineLoadRequest<'_>,
        discard_hot_context: impl FnOnce(),
    ) -> Result<LoadedEngine, LlamaEngineError> {
        let request_id = request.request_id;
        let model_path = request.model_path;
        let requested_gpu_layers = request.requested_gpu_layers;
        let auto_gpu_layer_candidates = request.auto_gpu_layer_candidates;
        let native_fit_plan = request.native_fit_plan;
        let gpu_config = &request.gpu_config;
        let strict_mode = request.strict_mode;
        let load_bundled_mtp = request.load_bundled_mtp;

        let mut guard = self
            .state
            .lock()
            .map_err(|_| LlamaEngineError::LockPoisoned)?;
        if guard.backend.is_none() {
            guard.backend = Some(shared_backend()?);
        }
        let supports_gpu = guard
            .backend
            .as_ref()
            .ok_or_else(|| LlamaEngineError::Backend("llama.cpp backend unavailable".into()))?
            .supports_gpu_offload();
        let gpu_backends = compiled_gpu_backends();
        let gpu_backend_label = if gpu_backends.is_empty() {
            "none".to_string()
        } else {
            gpu_backends.join(",")
        };
        guard.compiled_gpu_backends = gpu_backends
            .iter()
            .map(|value| (*value).to_string())
            .collect();
        guard.supports_gpu_offload = supports_gpu;
        if observer.is_some() {
            tracing::info!(
                compiled_gpu_backends = %gpu_backend_label,
                supports_gpu_offload = supports_gpu,
                "llama.cpp backend initialized"
            );
        }
        if let (Some(_), Some(requested)) = (observer, requested_gpu_layers)
            && requested > 0
            && !supports_gpu
        {
            if strict_mode {
                return Err(LlamaEngineError::StrictMode(format!(
                    "Strict mode is enabled and llamaGpuLayers={requested} requires GPU offload, but this build has no active GPU backend."
                )));
            }
            tracing::warn!(
                requested,
                "requested llama GPU layers but this build has no active GPU offload; using CPU layers only"
            );
        }
        let model_params_key = model_params_key(request);
        let mut should_reload = guard.model.is_none()
            || guard.model_path.as_deref() != Some(model_path)
            || guard.model_params_key.as_deref() != Some(&model_params_key);
        let reusing_loaded_smart_gpu_model = should_reload
            && native_fit_plan.is_none()
            && auto_gpu_layer_candidates.is_some()
            && guard.model.is_some()
            && guard.model_path.as_deref() == Some(model_path)
            && guard.backend_path_used == Some(BackendPath::GpuOffload)
            && guard.actual_gpu_layers_used.unwrap_or(0) > 0;
        if reusing_loaded_smart_gpu_model {
            should_reload = false;
            guard.model_params_key = Some(model_params_key.clone());
            if observer.is_some() {
                tracing::info!(
                    actual_gpu_layers_used = ?guard.actual_gpu_layers_used,
                    requested_candidates = ?auto_gpu_layer_candidates,
                    "reusing the loaded smart-offload GPU model despite a candidate change"
                );
            }
        }
        if !should_reload && observer.is_some() {
            tracing::info!(
                actual_gpu_layers_used = ?guard.actual_gpu_layers_used,
                backend_path = backend_label(guard.backend_path_used),
                "reusing the loaded llama.cpp model"
            );
        }
        if should_reload {
            discard_hot_context();
            if guard.model.is_some() {
                guard.drop_model();
                if observer.is_some() {
                    tracing::info!(
                        "dropped the previously loaded model before reload to free VRAM"
                    );
                }
            }

            let mut backend_path_used = BackendPath::Cpu;
            let mut actual_gpu_layers_used = None;
            let mut gpu_load_fallback_activated = false;
            let mut gpu_load_fallback_reason = None;
            let mut smart_gpu_layer_fallback_activated = false;
            guard.kqv_fallback_toast_shown = false;

            let emit = |backend: BackendPath, stage: ModelLoadStage, status: ModelLoadStatus| {
                if let Some(observer) = observer {
                    emit_model_load_progress(
                        observer,
                        request_id,
                        model_path,
                        backend.as_str(),
                        stage,
                        status,
                        0.0,
                        None,
                    );
                }
            };

            let model = if supports_gpu && requested_gpu_layers != Some(0) {
                if let Some(candidates) =
                    auto_gpu_layer_candidates.filter(|value| !value.is_empty())
                {
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
                            emit(
                                BackendPath::GpuOffload,
                                ModelLoadStage::GpuOffload,
                                ModelLoadStatus::Retrying,
                            );
                        }
                        let native = native_fit_plan
                            .filter(|plan| index == 0 && plan.n_gpu_layers == candidate);
                        match load_model_with_progress(
                            observer,
                            request_id,
                            model_path,
                            Some(candidate),
                            gpu_config,
                            BackendPath::GpuOffload,
                            ModelLoadStage::GpuOffload,
                            native.map(|plan| plan.model_params.as_ref().get_ref()),
                            load_bundled_mtp,
                        ) {
                            Ok(model) => {
                                backend_path_used = BackendPath::GpuOffload;
                                actual_gpu_layers_used = Some(candidate);
                                if observer.is_some() {
                                    if native.is_some() {
                                        tracing::info!(
                                            candidate,
                                            "loaded the model with llama.cpp native fit"
                                        );
                                    } else if smart_gpu_layer_fallback_activated {
                                        tracing::warn!(
                                            candidate,
                                            "smart GPU offload backed off after earlier load failures"
                                        );
                                    } else {
                                        tracing::info!(
                                            candidate,
                                            "loaded the model with smart GPU offload"
                                        );
                                    }
                                }
                                resolved_model = Some(Arc::new(model));
                                break;
                            }
                            Err(error) => {
                                if observer.is_some() {
                                    tracing::warn!(
                                        candidate,
                                        %error,
                                        "smart GPU offload attempt failed"
                                    );
                                }
                                gpu_attempt_error = Some(error.to_string());
                            }
                        }
                    }

                    if let Some(model) = resolved_model {
                        model
                    } else {
                        if strict_mode {
                            return Err(LlamaEngineError::StrictMode(
                                "Strict mode is enabled, so llama.cpp will not fall back after smart GPU offload failure.".into(),
                            ));
                        }
                        let cpu_stage = if attempted_gpu_candidate {
                            ModelLoadStage::CpuFallback
                        } else {
                            ModelLoadStage::Cpu
                        };
                        if attempted_gpu_candidate {
                            gpu_load_fallback_activated = true;
                            gpu_load_fallback_reason.clone_from(&gpu_attempt_error);
                            if let Some(observer) = observer {
                                emit(
                                    BackendPath::Cpu,
                                    ModelLoadStage::CpuFallback,
                                    ModelLoadStatus::Retrying,
                                );
                                tracing::warn!(
                                    ?candidates,
                                    error = gpu_attempt_error
                                        .as_deref()
                                        .unwrap_or("unknown GPU load error"),
                                    "smart GPU offload exhausted its GPU layer candidates, falling back to CPU"
                                );
                                observer.gpu_fallback();
                            }
                        }
                        actual_gpu_layers_used = Some(0);
                        Arc::new(
                            load_model_with_progress(
                                observer,
                                request_id,
                                model_path,
                                Some(0),
                                &LlamaGpuConfig::default(),
                                BackendPath::Cpu,
                                cpu_stage,
                                None,
                                load_bundled_mtp,
                            )
                            .inspect_err(|_| {
                                emit(BackendPath::Cpu, cpu_stage, ModelLoadStatus::Failed);
                            })?,
                        )
                    }
                } else {
                    match load_model_with_progress(
                        observer,
                        request_id,
                        model_path,
                        requested_gpu_layers,
                        gpu_config,
                        BackendPath::GpuOffload,
                        ModelLoadStage::GpuOffload,
                        native_fit_plan.map(|plan| plan.model_params.as_ref().get_ref()),
                        load_bundled_mtp,
                    ) {
                        Ok(model) => {
                            backend_path_used = BackendPath::GpuOffload;
                            actual_gpu_layers_used = requested_gpu_layers;
                            if observer.is_some() {
                                tracing::info!(
                                    mode = requested_gpu_layers.map_or_else(
                                        || "llama-default".to_string(),
                                        |value| value.to_string()
                                    ),
                                    "loaded the model with GPU mode"
                                );
                            }
                            Arc::new(model)
                        }
                        Err(error) => {
                            if strict_mode {
                                if observer.is_some() {
                                    emit(
                                        BackendPath::GpuOffload,
                                        ModelLoadStage::GpuOffload,
                                        ModelLoadStatus::Failed,
                                    );
                                    tracing::warn!(
                                        %error,
                                        "GPU model load failed with strict mode enabled; refusing CPU fallback"
                                    );
                                }
                                return Err(LlamaEngineError::StrictMode(format!(
                                    "Strict mode is enabled, so llama.cpp will not fall back to CPU after GPU load failure: {error}"
                                )));
                            }
                            gpu_load_fallback_activated = true;
                            gpu_load_fallback_reason = Some(error.to_string());
                            if let Some(observer) = observer {
                                emit(
                                    BackendPath::Cpu,
                                    ModelLoadStage::CpuFallback,
                                    ModelLoadStatus::Retrying,
                                );
                                tracing::warn!(%error, "GPU model load failed, falling back to CPU");
                                observer.gpu_fallback();
                            }
                            actual_gpu_layers_used = Some(0);
                            Arc::new(
                                load_model_with_progress(
                                    observer,
                                    request_id,
                                    model_path,
                                    Some(0),
                                    &LlamaGpuConfig::default(),
                                    BackendPath::Cpu,
                                    ModelLoadStage::CpuFallback,
                                    None,
                                    load_bundled_mtp,
                                )
                                .inspect_err(|_| {
                                    emit(
                                        BackendPath::Cpu,
                                        ModelLoadStage::CpuFallback,
                                        ModelLoadStatus::Failed,
                                    );
                                })?,
                            )
                        }
                    }
                }
            } else {
                actual_gpu_layers_used = Some(0);
                Arc::new(
                    load_model_with_progress(
                        observer,
                        request_id,
                        model_path,
                        Some(0),
                        &LlamaGpuConfig::default(),
                        BackendPath::Cpu,
                        ModelLoadStage::Cpu,
                        None,
                        load_bundled_mtp,
                    )
                    .inspect_err(|_| {
                        emit(
                            BackendPath::Cpu,
                            ModelLoadStage::Cpu,
                            ModelLoadStatus::Failed,
                        );
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

        let mmproj_path = request.mmproj_path;
        let mmproj_changed = should_reload
            || guard.mmproj_path.as_deref() != mmproj_path
            || (mmproj_path.is_some() && guard.mtmd_ctx.is_none());
        if mmproj_changed {
            guard.mtmd_ctx = None;
            guard.mmproj_path = None;
            if let Some(mmproj_path) = mmproj_path {
                if !Path::new(mmproj_path).exists() {
                    return Err(LlamaEngineError::Sidecar(format!(
                        "mmproj file not found: {mmproj_path}"
                    )));
                }
                let model = guard.model.as_ref().ok_or_else(|| {
                    LlamaEngineError::Sidecar("llama.cpp model unavailable for mtmd init".into())
                })?;
                let mtmd = MtmdContext::init_from_file(
                    mmproj_path,
                    model.as_ref(),
                    &MtmdContextParams::default(),
                )
                .map_err(|error| {
                    LlamaEngineError::Sidecar(format!(
                        "Failed to initialize llama.cpp mtmd context from {mmproj_path}: {error}"
                    ))
                })?;
                if observer.is_some() {
                    tracing::info!(
                        vision = mtmd.support_vision(),
                        audio = mtmd.support_audio(),
                        "mtmd loaded"
                    );
                }
                guard.mtmd_ctx = Some(Arc::new(mtmd));
                guard.mmproj_path = Some(mmproj_path.to_string());
            }
        }

        let mtp_model_path = request.mtp_model_path;
        let mtp_model_config_key = mtp_model_path.map(|path| {
            format!(
                "path={path};gpu={};device={:?}",
                request.mtp_drafter_on_gpu, request.mtp_gpu_device_id
            )
        });
        let mtp_changed = should_reload
            || guard.mtp_model_path.as_deref() != mtp_model_path
            || guard.mtp_model_config_key != mtp_model_config_key
            || (mtp_model_path.is_some() && guard.mtp_model.is_none());
        if mtp_changed {
            guard.mtp_model = None;
            guard.mtp_model_path = None;
            guard.mtp_model_config_key = None;
            if let Some(mtp_path) = mtp_model_path {
                if !Path::new(mtp_path).exists() {
                    return Err(LlamaEngineError::Sidecar(format!(
                        "MTP draft model file not found: {mtp_path}"
                    )));
                }
                let drafter_gpu_layers = if request.mtp_drafter_on_gpu
                    && guard.backend_path_used == Some(BackendPath::GpuOffload)
                {
                    Some(1000)
                } else {
                    Some(0)
                };
                let mtp_gpu_config = LlamaGpuConfig {
                    device_ids: request.mtp_gpu_device_id.into_iter().collect(),
                    ..LlamaGpuConfig::default()
                };
                let drafter_backend = guard.backend_path_used.unwrap_or(BackendPath::Cpu);
                let drafter_result = load_model_with_progress(
                    None,
                    None,
                    mtp_path,
                    drafter_gpu_layers,
                    &mtp_gpu_config,
                    drafter_backend,
                    ModelLoadStage::Finalizing,
                    None,
                    true,
                );
                let (drafter, drafter_gpu_layers) = match drafter_result {
                    Ok(drafter) => (drafter, drafter_gpu_layers),
                    Err(error)
                        if drafter_gpu_layers != Some(0) && request.mtp_gpu_fallback_allowed =>
                    {
                        if observer.is_some() {
                            tracing::warn!(
                                %error,
                                "MTP auto GPU placement failed; retrying the draft model on CPU"
                            );
                        }
                        (
                            load_model_with_progress(
                                None,
                                None,
                                mtp_path,
                                Some(0),
                                &LlamaGpuConfig::default(),
                                BackendPath::Cpu,
                                ModelLoadStage::Finalizing,
                                None,
                                true,
                            )?,
                            Some(0),
                        )
                    }
                    Err(error) => return Err(error),
                };
                if observer.is_some() {
                    tracing::info!(
                        gpu_layers = ?drafter_gpu_layers,
                        gpu_device = ?request.mtp_gpu_device_id,
                        "MTP draft model loaded"
                    );
                }
                guard.mtp_model = Some(Arc::new(drafter));
                guard.mtp_model_path = Some(mtp_path.to_string());
                guard.mtp_model_config_key = mtp_model_config_key;
            }
        }

        Ok(LoadedEngine {
            model_reloaded: should_reload,
            backend: guard
                .backend
                .clone()
                .ok_or_else(|| LlamaEngineError::Backend("llama.cpp backend unavailable".into()))?,
            model: guard
                .model
                .clone()
                .ok_or_else(|| LlamaEngineError::Load("llama.cpp model unavailable".into()))?,
            backend_path_used: guard.backend_path_used,
            actual_gpu_layers_used: guard.actual_gpu_layers_used,
            gpu_load_fallback_activated: guard.gpu_load_fallback_activated,
            gpu_load_fallback_reason: guard.gpu_load_fallback_reason.clone(),
            smart_gpu_layer_fallback_activated: guard.smart_gpu_layer_fallback_activated,
            compiled_gpu_backends: guard.compiled_gpu_backends.clone(),
            supports_gpu_offload: guard.supports_gpu_offload,
            mtmd_ctx: guard.mtmd_ctx.clone(),
            mtp_model: guard.mtp_model.clone(),
        })
    }

    /// Drops the loaded model and its sidecars.
    pub fn unload(&self) -> Result<(), LlamaEngineError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| LlamaEngineError::LockPoisoned)?;
        if guard.model.is_some() {
            guard.drop_model();
            guard.kqv_fallback_toast_shown = false;
            tracing::info!("unloaded llama.cpp model");
        }
        Ok(())
    }

    /// Unloads when a different model is loaded; reports whether it did.
    pub fn unload_if_model_differs(&self, model_path: &str) -> Result<bool, LlamaEngineError> {
        let loaded_path = self
            .state
            .lock()
            .ok()
            .and_then(|guard| guard.model_path.clone());
        match loaded_path {
            Some(loaded) if loaded != model_path => {
                self.unload()?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Whether the KV-offload fallback notice should be shown for this model
    /// (once per load of the same model).
    pub fn consume_kqv_fallback_toast(&self, model_path: &str) -> Result<bool, LlamaEngineError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| LlamaEngineError::LockPoisoned)?;
        if guard.model_path.as_deref() != Some(model_path) {
            return Ok(true);
        }
        if guard.kqv_fallback_toast_shown {
            return Ok(false);
        }
        guard.kqv_fallback_toast_shown = true;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_progress_ranges_follow_the_split_after_the_cpu_layers() {
        let ranges = compute_gpu_progress_ranges(
            &["A".to_string(), "B".to_string()],
            &[1.0, 3.0],
            Some(40),
            Some(79),
        );
        assert_eq!(ranges.len(), 2);
        assert!((ranges[0].1 - 0.5).abs() < 1e-6);
        assert!((ranges[0].2 - 0.625).abs() < 1e-6);
        assert!((ranges[1].2 - 1.0).abs() < 1e-6);
        assert!(compute_gpu_progress_ranges(&["A".to_string()], &[1.0], None, Some(10)).is_empty());
        let payload = gpu_progress_payload(&ranges, 0.5625).expect("payload");
        assert_eq!(payload[0].percent, 50);
        assert_eq!(payload[1].percent, 0);
    }

    #[test]
    fn load_stage_follows_the_backend_and_fallback() {
        assert_eq!(
            model_load_stage_for_backend(Some(BackendPath::GpuOffload), false),
            ModelLoadStage::GpuOffload
        );
        assert_eq!(
            model_load_stage_for_backend(Some(BackendPath::Cpu), true),
            ModelLoadStage::CpuFallback
        );
        assert_eq!(
            model_load_stage_for_backend(Some(BackendPath::Cpu), false),
            ModelLoadStage::Cpu
        );
        assert_eq!(
            model_load_stage_for_backend(None, false),
            ModelLoadStage::Finalizing
        );
        assert_eq!(ModelLoadStage::CpuFallback as u8, 2);
        assert_eq!(ModelLoadStatus::Failed as u8, 3);
    }

    #[test]
    fn params_key_names_every_load_parameter() {
        let request = EngineLoadRequest {
            request_id: None,
            model_path: "/models/a.gguf",
            requested_gpu_layers: None,
            auto_gpu_layer_candidates: Some(&[40, 30, 0]),
            native_fit_plan: None,
            gpu_config: LlamaGpuConfig {
                multi_gpu_enabled: true,
                device_ids: vec![0, 1],
                tensor_split: vec![0.25, 0.75],
                ..LlamaGpuConfig::default()
            },
            strict_mode: false,
            mmproj_path: None,
            load_bundled_mtp: true,
            mtp_model_path: None,
            mtp_drafter_on_gpu: false,
            mtp_gpu_fallback_allowed: false,
            mtp_gpu_device_id: None,
        };
        assert_eq!(
            model_params_key(&request),
            "requested_gpu_layers=smart:40,30,0;native_fit=none;load_mtp=true;strict_mode=false;multi_gpu=true;devices=0,1;distribution=balanced;tensor_split=0.2500,0.7500;main_gpu=auto"
        );
    }
}

#[cfg(test)]
mod real_model_load {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        events: Mutex<Vec<ModelLoadProgress>>,
    }

    impl EngineObserver for Recorder {
        fn model_load_progress(&self, progress: ModelLoadProgress) {
            if let Ok(mut events) = self.events.lock() {
                events.push(progress);
            }
        }

        fn gpu_fallback(&self) {}
    }

    #[test]
    #[ignore = "needs a local GGUF model in LETTUCE_PLAN_MODEL"]
    fn loads_on_the_cpu_then_reuses_the_model() {
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let engine = LlamaEngine::new();
        let recorder = Recorder::default();
        let request = EngineLoadRequest {
            request_id: Some("smoke"),
            model_path: &path,
            requested_gpu_layers: Some(0),
            auto_gpu_layer_candidates: None,
            native_fit_plan: None,
            gpu_config: LlamaGpuConfig::default(),
            strict_mode: false,
            mmproj_path: None,
            load_bundled_mtp: false,
            mtp_model_path: None,
            mtp_drafter_on_gpu: false,
            mtp_gpu_fallback_allowed: false,
            mtp_gpu_device_id: None,
        };
        let mut discarded = 0;
        let first = engine
            .load(Some(&recorder), &request, || discarded += 1)
            .expect("first load");
        assert!(first.model_reloaded);
        assert_eq!(first.backend_path_used, Some(BackendPath::Cpu));
        assert_eq!(first.actual_gpu_layers_used, Some(0));
        let second = engine
            .load(Some(&recorder), &request, || discarded += 1)
            .expect("second load");
        assert!(!second.model_reloaded);
        assert_eq!(discarded, 1);
        let events = recorder.events.lock().expect("events");
        assert_eq!(events.first().map(|event| event.percent), Some(0));
        assert!(
            events
                .iter()
                .all(|event| event.stage == ModelLoadStage::Cpu)
        );
        assert!(
            engine
                .unload_if_model_differs("/other.gguf")
                .expect("unload")
        );
    }
}
