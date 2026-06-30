use super::engine::{shared_backend, using_rocm_backend};
use super::offload::{
    compute_recommended_context_for_gpu_layers, load_model_metadata, plan_multi_gpu_distribution,
    plan_smart_gpu_offload, LlamaModelMetadata,
};
use crate::chat_manager::types::GpuLayerAssignment;
use super::*;
use llama_cpp_sys_2::{
    ggml_backend_dev_count, ggml_backend_dev_get, ggml_backend_dev_memory, ggml_backend_dev_type,
    GGML_BACKEND_DEVICE_TYPE_ACCEL, GGML_BACKEND_DEVICE_TYPE_GPU, GGML_BACKEND_DEVICE_TYPE_IGPU,
};
#[cfg(target_os = "windows")]
use windows::core::Interface;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIAdapter3, IDXGIFactory6, DXGI_ADAPTER_FLAG_SOFTWARE,
    DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LlamaCppContextInfo {
    max_context_length: u32,
    recommended_context_length: Option<u32>,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    model_size_bytes: Option<u64>,
    layer_count: Option<u32>,
    supports_gpu_offload: Option<bool>,
    selected_gpu_device_ids: Option<Vec<usize>>,
    per_device_vram: Option<Vec<PerDeviceVram>>,
    estimated_placement: Option<EstimatedPlacement>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerDeviceVram {
    index: usize,
    memory_free: u64,
    memory_total: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EstimatedPlacement {
    total_gpu_layers: u32,
    per_device_layers: Vec<u32>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LlamaGpuDeviceInfo {
    index: usize,
    name: String,
    description: String,
    backend: String,
    memory_total: u64,
    memory_free: u64,
    device_type: String,
}

fn push_unique_u32(out: &mut Vec<u32>, value: u32) {
    if !out.contains(&value) {
        out.push(value);
    }
}

pub(super) fn context_attempt_candidates(
    initial_ctx_size: u32,
    prompt_tokens: usize,
    requested_context: Option<u32>,
    llama_batch_size: u32,
) -> Vec<(u32, u32)> {
    let minimum_ctx = (prompt_tokens as u32).saturating_add(1).max(1);
    let mut ctx_candidates = Vec::new();
    push_unique_u32(&mut ctx_candidates, initial_ctx_size.max(minimum_ctx));

    let mut scaled = if requested_context.is_some() {
        vec![initial_ctx_size.saturating_mul(3) / 4, initial_ctx_size / 2]
    } else {
        vec![
            initial_ctx_size.saturating_mul(3) / 4,
            initial_ctx_size / 2,
            initial_ctx_size / 3,
            initial_ctx_size / 4,
        ]
    };
    scaled.extend([8192, 4096, 3072, 2048, 1024, 768, 512]);

    for candidate in scaled {
        let clamped = candidate.max(minimum_ctx);
        if clamped > 0 {
            push_unique_u32(&mut ctx_candidates, clamped);
        }
    }

    let mut attempts = Vec::new();
    for ctx in ctx_candidates {
        let primary_batch = ctx.min(llama_batch_size).max(1);
        if !attempts.contains(&(ctx, primary_batch)) {
            attempts.push((ctx, primary_batch));
        }
        let reduced_batch = (primary_batch / 2).max(1);
        if reduced_batch != primary_batch && !attempts.contains(&(ctx, reduced_batch)) {
            attempts.push((ctx, reduced_batch));
        }
    }
    attempts
}

pub(super) fn is_likely_context_oom_error(raw_error: &str) -> bool {
    let lower = raw_error.to_ascii_lowercase();
    lower.contains("null reference from llama.cpp")
        || lower.contains("out of memory")
        || lower.contains("oom")
        || lower.contains("alloc")
        || lower.contains("reserve")
        || lower.contains("failed to create")
}

pub(super) fn context_error_detail(
    raw_error: &str,
    ctx_size: u32,
    n_batch: u32,
    resolved_offload_kqv: Option<bool>,
    llama_offload_kqv: Option<bool>,
    recommended_ctx: Option<u32>,
    llama_kv_type_raw: Option<&str>,
) -> String {
    if let Some(kv_type_raw) = llama_kv_type_raw {
        return format!(
            "llama.cpp rejected llamaKvType='{}' while creating the context (ctx={}, batch={}, offload_kqv={:?}): {}",
            kv_type_raw, ctx_size, n_batch, resolved_offload_kqv, raw_error
        );
    }

    if raw_error.contains("null reference from llama.cpp") {
        if let Some(recommended) = recommended_ctx {
            if recommended > 0 && ctx_size > recommended {
                return format!(
                    "Likely memory allocation failure for context {}. Recommended <= {} tokens for current {} budget.",
                    ctx_size,
                    recommended,
                    if llama_offload_kqv == Some(true) {
                        "VRAM"
                    } else {
                        "RAM"
                    }
                );
            }
        }
        return "Likely memory allocation failure (OOM) in llama.cpp. Try lower context length, lower llamaBatchSize, or a denser KV type (q8_0/q4_0).".to_string();
    }

    raw_error.to_string()
}

pub(crate) fn get_available_memory_bytes() -> Option<u64> {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    Some(sys.available_memory())
}

fn choose_effective_vram_bytes(
    ggml_free_bytes: Option<u64>,
    platform_cap_bytes: Option<u64>,
) -> Option<u64> {
    match (
        ggml_free_bytes.filter(|value| *value > 0),
        platform_cap_bytes.filter(|value| *value > 0),
    ) {
        (Some(ggml_free), Some(platform_cap)) => Some(ggml_free.min(platform_cap)),
        (Some(ggml_free), None) => Some(ggml_free),
        (None, Some(platform_cap)) => Some(platform_cap),
        (None, None) => None,
    }
}

fn ggml_available_vram_bytes() -> Option<u64> {
    let mut max_free: u64 = 0;
    // SAFETY: read-only ggml backend device enumeration and memory queries.
    unsafe {
        let count = ggml_backend_dev_count();
        for i in 0..count {
            let dev = ggml_backend_dev_get(i);
            if dev.is_null() {
                continue;
            }
            let dev_type = ggml_backend_dev_type(dev);
            let is_gpu_like = dev_type == GGML_BACKEND_DEVICE_TYPE_GPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_IGPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_ACCEL;
            if !is_gpu_like {
                continue;
            }
            let mut free: usize = 0;
            let mut total: usize = 0;
            ggml_backend_dev_memory(dev, &mut free, &mut total);
            if total == 0 {
                continue;
            }
            let free_u64 = free as u64;
            if free_u64 > max_free {
                max_free = free_u64;
            }
        }
    }
    if max_free > 0 {
        Some(max_free)
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn windows_local_vram_cap_bytes() -> Option<u64> {
    // SAFETY: read-only DXGI factory/adapter enumeration and local-memory queries.
    unsafe {
        let factory: IDXGIFactory6 = CreateDXGIFactory1().ok()?;
        let mut best: u64 = 0;
        let mut index: u32 = 0;

        loop {
            let adapter: IDXGIAdapter1 = match factory.EnumAdapters1(index) {
                Ok(adapter) => adapter,
                Err(_) => break,
            };
            index = index.saturating_add(1);

            let desc = match adapter.GetDesc1() {
                Ok(desc) => desc,
                Err(_) => continue,
            };
            if desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                continue;
            }

            let dedicated_bytes = desc.DedicatedVideoMemory as u64;
            if dedicated_bytes == 0 {
                continue;
            }

            let local_available_bytes = adapter
                .cast::<IDXGIAdapter3>()
                .ok()
                .and_then(|adapter3: IDXGIAdapter3| {
                    let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                    adapter3
                        .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
                        .ok()?;
                    Some(
                        info.Budget
                            .saturating_sub(info.CurrentUsage)
                            .min(dedicated_bytes),
                    )
                })
                .unwrap_or(dedicated_bytes);

            if local_available_bytes > best {
                best = local_available_bytes;
            }
        }

        (best > 0).then_some(best)
    }
}

#[cfg(not(target_os = "windows"))]
fn windows_local_vram_cap_bytes() -> Option<u64> {
    None
}

pub(crate) fn get_available_vram_bytes() -> Option<u64> {
    choose_effective_vram_bytes(ggml_available_vram_bytes(), windows_local_vram_cap_bytes())
}

pub(crate) fn list_gpu_devices() -> Vec<LlamaGpuDeviceInfo> {
    llama_cpp_2::list_llama_ggml_backend_devices()
        .into_iter()
        .filter(|device| {
            matches!(
                device.device_type,
                llama_cpp_2::LlamaBackendDeviceType::Gpu
                    | llama_cpp_2::LlamaBackendDeviceType::Accelerator
            )
        })
        .map(|device| LlamaGpuDeviceInfo {
            index: device.index,
            name: device.name,
            description: device.description,
            backend: device.backend,
            memory_total: device.memory_total as u64,
            memory_free: device.memory_free as u64,
            device_type: format!("{:?}", device.device_type),
        })
        .collect()
}

pub(crate) fn get_available_vram_bytes_for_devices(device_ids: &[usize]) -> Option<u64> {
    if device_ids.is_empty() {
        return None;
    }
    let mut total_free: u64 = 0;
    let mut found = false;
    unsafe {
        let count = ggml_backend_dev_count();
        for device_id in device_ids {
            if *device_id >= count {
                continue;
            }
            let dev = ggml_backend_dev_get(*device_id);
            if dev.is_null() {
                continue;
            }
            let dev_type = ggml_backend_dev_type(dev);
            let is_gpu_like = dev_type == GGML_BACKEND_DEVICE_TYPE_GPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_ACCEL;
            if !is_gpu_like {
                continue;
            }
            let mut free: usize = 0;
            let mut total: usize = 0;
            ggml_backend_dev_memory(dev, &mut free, &mut total);
            if total == 0 {
                continue;
            }
            found = true;
            total_free = total_free.saturating_add(free as u64);
        }
    }
    if found && total_free > 0 {
        Some(total_free)
    } else {
        None
    }
}

/// Per-device free/total VRAM for the explicitly selected device ids, preserving
/// the caller's order. Integrated GPUs are skipped (never used for multi-GPU).
/// Each tuple is `(device_id, free_bytes, total_bytes)`.
pub(crate) fn get_per_device_free_vram(device_ids: &[usize]) -> Vec<(usize, u64, u64)> {
    let mut out = Vec::new();
    unsafe {
        let count = ggml_backend_dev_count();
        for device_id in device_ids {
            if *device_id >= count {
                continue;
            }
            let dev = ggml_backend_dev_get(*device_id);
            if dev.is_null() {
                continue;
            }
            let dev_type = ggml_backend_dev_type(dev);
            let is_gpu_like = dev_type == GGML_BACKEND_DEVICE_TYPE_GPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_ACCEL;
            if !is_gpu_like {
                continue;
            }
            let mut free: usize = 0;
            let mut total: usize = 0;
            ggml_backend_dev_memory(dev, &mut free, &mut total);
            if total == 0 {
                continue;
            }
            out.push((*device_id, free as u64, total as u64));
        }
    }
    out
}

/// Detect if the system uses unified memory (shared RAM/VRAM).
/// True on Apple Silicon (macOS aarch64) or when only iGPU devices are found.
pub(crate) fn is_unified_memory() -> bool {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        return true;
    }

    let mut found_gpu = false;
    let mut all_igpu = true;
    unsafe {
        let count = ggml_backend_dev_count();
        for i in 0..count {
            let dev = ggml_backend_dev_get(i);
            if dev.is_null() {
                continue;
            }
            let dev_type = ggml_backend_dev_type(dev);
            if dev_type == GGML_BACKEND_DEVICE_TYPE_GPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_IGPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_ACCEL
            {
                found_gpu = true;
                if dev_type != GGML_BACKEND_DEVICE_TYPE_IGPU {
                    all_igpu = false;
                }
            }
        }
    }
    found_gpu && all_igpu
}

fn kv_bytes_per_value(llama_kv_type: Option<&str>) -> f64 {
    match llama_kv_type
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("f32") => 4.0,
        Some("f16") => 2.0,
        Some("q8_1") | Some("q8_0") => 1.0,
        Some("q6_k") => 0.75,
        Some("q5_k") | Some("q5_1") | Some("q5_0") => 0.625,
        Some("q4_k") | Some("q4_1") | Some("q4_0") => 0.5,
        Some("q3_k") | Some("iq3_s") | Some("iq3_xxs") => 0.375,
        Some("q2_k") | Some("iq2_xs") | Some("iq2_xxs") | Some("iq1_s") => 0.25,
        Some("iq4_nl") => 0.5,
        _ => 2.0,
    }
}

fn estimate_kv_bytes_per_token(model: &LlamaModel, llama_kv_type: Option<&str>) -> Option<u64> {
    let n_layer = u64::from(model.n_layer());
    let n_embd = u64::try_from(model.n_embd()).ok()?;
    let n_head = u64::from(model.n_head()).max(1);
    let n_head_kv = u64::from(model.n_head_kv()).max(1);
    let gqa_correction = n_head_kv as f64 / n_head as f64;
    let effective_n_embd = (n_embd as f64 * gqa_correction) as u64;
    let bytes_per_value = kv_bytes_per_value(llama_kv_type);
    let bytes = (n_layer as f64) * (effective_n_embd as f64) * 2.0 * bytes_per_value;
    Some(bytes.max(0.0) as u64)
}

fn estimate_kv_bytes_per_token_from_metadata(
    metadata: &LlamaModelMetadata,
    llama_kv_type: Option<&str>,
) -> Option<u64> {
    let n_layer = u64::from(metadata.layer_count.max(1));
    let n_embd = metadata.n_embd.max(1);
    let n_head = metadata.n_head.max(1);
    let n_head_kv = metadata.n_head_kv.max(1);
    let gqa_correction = n_head_kv as f64 / n_head as f64;
    let effective_n_embd = (n_embd as f64 * gqa_correction) as u64;
    let bytes_per_value = kv_bytes_per_value(llama_kv_type);
    let bytes = (n_layer as f64) * (effective_n_embd as f64) * 2.0 * bytes_per_value;
    Some(bytes.max(0.0) as u64)
}

fn default_memory_reserve_bytes(available_memory_bytes: u64) -> u64 {
    (available_memory_bytes / 5).max(512 * 1024 * 1024)
}

fn resident_weight_bytes(model_size: u64, total_layers: u32, resident_layers: u32) -> u64 {
    let total = u128::from(total_layers.max(1));
    let resident = u128::from(resident_layers.min(total_layers));
    ((u128::from(model_size) * resident) / total) as u64
}

fn ram_budget_for_context(model: &LlamaModel, available_memory_bytes: u64, gpu_layers: u32) -> u64 {
    let total_layers = model.n_layer();
    let cpu_layers = total_layers.saturating_sub(gpu_layers);
    let cpu_resident = resident_weight_bytes(model.size(), total_layers, cpu_layers);
    let reserve = default_memory_reserve_bytes(available_memory_bytes);
    available_memory_bytes.saturating_sub(cpu_resident.saturating_add(reserve))
}

fn cpu_fallback_headroom_bytes(base_budget: u64, available_memory_bytes: u64) -> u64 {
    let availability_headroom = (available_memory_bytes / 10).max(256 * 1024 * 1024);
    let budget_headroom = (base_budget / 4).max(128 * 1024 * 1024);
    availability_headroom
        .min(base_budget)
        .max(budget_headroom.min(base_budget))
}

fn safe_cpu_context_from_budget(
    base_budget: u64,
    available_memory_bytes: u64,
    kv_bytes_per_token: u64,
    max_context_length: u32,
    requested_context: Option<u32>,
) -> u32 {
    let base_context = (base_budget / kv_bytes_per_token).min(u64::from(max_context_length)) as u32;
    let requested_or_base_context = requested_context
        .unwrap_or(base_context)
        .min(max_context_length)
        .max(1);

    let extra_cpu_headroom = cpu_fallback_headroom_bytes(base_budget, available_memory_bytes);
    let safe_budget = base_budget.saturating_sub(extra_cpu_headroom);
    (safe_budget / kv_bytes_per_token)
        .min(u64::from(requested_or_base_context))
        .max(1) as u32
}

pub(super) fn compute_recommended_context(
    model: &LlamaModel,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    max_context_length: u32,
    gpu_layers: u32,
    llama_offload_kqv: Option<bool>,
    llama_kv_type: Option<&str>,
) -> Option<u32> {
    let available_for_ctx = if llama_offload_kqv == Some(true) {
        let vram = available_vram_bytes?;
        let gpu_resident = resident_weight_bytes(model.size(), model.n_layer(), gpu_layers);
        let reserve = default_memory_reserve_bytes(vram);
        vram.saturating_sub(gpu_resident.saturating_add(reserve))
    } else {
        let ram = available_memory_bytes?;
        ram_budget_for_context(model, ram, gpu_layers)
    };
    let kv_bytes_per_token = estimate_kv_bytes_per_token(model, llama_kv_type)?;
    if kv_bytes_per_token == 0 {
        return None;
    }
    let mut recommended = available_for_ctx / kv_bytes_per_token;
    if recommended > u64::from(max_context_length) {
        recommended = u64::from(max_context_length);
    }
    Some(recommended as u32)
}

pub(super) fn compute_cpu_fallback_limits(
    model: &LlamaModel,
    available_memory_bytes: Option<u64>,
    max_context_length: u32,
    gpu_layers: u32,
    llama_kv_type: Option<&str>,
    requested_context: Option<u32>,
    requested_batch_size: u32,
) -> Option<(u32, u32)> {
    let available_memory_bytes = available_memory_bytes?;
    let kv_bytes_per_token = estimate_kv_bytes_per_token(model, llama_kv_type)?;
    if kv_bytes_per_token == 0 {
        return None;
    }

    let base_budget = ram_budget_for_context(model, available_memory_bytes, gpu_layers);
    let requested_batch_size = requested_batch_size.max(1);
    let base_context = (base_budget / kv_bytes_per_token).min(u64::from(max_context_length)) as u32;
    let requested_or_base_context = requested_context
        .unwrap_or(base_context)
        .min(max_context_length)
        .max(1);
    let safe_context = safe_cpu_context_from_budget(
        base_budget,
        available_memory_bytes,
        kv_bytes_per_token,
        max_context_length,
        requested_context,
    );

    let safe_batch = u64::from(requested_batch_size)
        .saturating_mul(u64::from(safe_context))
        .checked_div(u64::from(requested_or_base_context))
        .unwrap_or(u64::from(requested_batch_size))
        .max(1)
        .min(u64::from(requested_batch_size)) as u32;

    Some((safe_context, safe_batch.max(1)))
}

fn compute_cpu_safe_recommended_context_for_metadata(
    metadata: &LlamaModelMetadata,
    available_memory_bytes: Option<u64>,
    llama_kv_type: Option<&str>,
    requested_context: Option<u32>,
) -> Option<u32> {
    let available_memory_bytes = available_memory_bytes?;
    let kv_bytes_per_token = estimate_kv_bytes_per_token_from_metadata(metadata, llama_kv_type)?;
    if kv_bytes_per_token == 0 {
        return None;
    }

    let reserve = default_memory_reserve_bytes(available_memory_bytes);
    let base_budget =
        available_memory_bytes.saturating_sub(metadata.model_size_bytes.saturating_add(reserve));

    Some(safe_cpu_context_from_budget(
        base_budget,
        available_memory_bytes,
        kv_bytes_per_token,
        metadata.max_context_length.max(1),
        requested_context,
    ))
}

#[cfg(test)]
mod tests {
    use super::{choose_effective_vram_bytes, cpu_fallback_headroom_bytes};

    #[test]
    fn cpu_fallback_headroom_does_not_exceed_budget() {
        let budget = 3_u64 * 1024 * 1024 * 1024;
        let available = 12_u64 * 1024 * 1024 * 1024;
        let headroom = cpu_fallback_headroom_bytes(budget, available);

        assert!(headroom > 0);
        assert!(headroom < budget);
    }

    #[test]
    fn cpu_fallback_headroom_keeps_small_budget_usable() {
        let budget = 700_u64 * 1024 * 1024;
        let available = 8_u64 * 1024 * 1024 * 1024;
        let headroom = cpu_fallback_headroom_bytes(budget, available);

        assert!(headroom < budget);
        assert!(budget - headroom >= 128_u64 * 1024 * 1024);
    }

    #[test]
    fn windows_vram_cap_clamps_inflated_backend_free_memory() {
        let ggml_free = Some(14_u64 * 1024 * 1024 * 1024);
        let windows_cap = Some(4_u64 * 1024 * 1024 * 1024);

        assert_eq!(
            choose_effective_vram_bytes(ggml_free, windows_cap),
            Some(4_u64 * 1024 * 1024 * 1024)
        );
    }

    #[test]
    fn windows_vram_cap_preserves_backend_value_without_platform_cap() {
        let ggml_free = Some(3_u64 * 1024 * 1024 * 1024);

        assert_eq!(
            choose_effective_vram_bytes(ggml_free, None),
            Some(3_u64 * 1024 * 1024 * 1024)
        );
    }
}

pub(crate) async fn llamacpp_context_info(
    app: AppHandle,
    model_path: String,
    llama_offload_kqv: Option<bool>,
    llama_kv_type: Option<String>,
    llama_gpu_layers: Option<u32>,
    llama_multi_gpu_enabled: Option<bool>,
    llama_gpu_device_ids: Option<Vec<usize>>,
    llama_gpu_distribution_mode: Option<String>,
    llama_gpu_manual_layers: Option<Vec<GpuLayerAssignment>>,
    llama_main_gpu: Option<i32>,
    llama_kv_placement: Option<String>,
    llama_priority_vram_limit_bytes: Option<u64>,
    llama_mmproj_path: Option<String>,
    llama_mtp_enabled: Option<bool>,
    llama_mtp_model_path: Option<String>,
) -> Result<LlamaCppContextInfo, String> {
    let _ = app;
    let _ = llama_main_gpu;
    if model_path.trim().is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "llama.cpp model path is empty",
        ));
    }
    if !Path::new(&model_path).exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("llama.cpp model path not found: {}", model_path),
        ));
    }

    let metadata = load_model_metadata(&model_path)?;
    let max_ctx = metadata.max_context_length.max(1);
    let available_memory_bytes = get_available_memory_bytes();
    let selected_gpu_device_ids = llama_gpu_device_ids.unwrap_or_default();
    let multi_gpu_active =
        llama_multi_gpu_enabled == Some(true) && selected_gpu_device_ids.len() >= 2;
    let available_vram_bytes = if multi_gpu_active {
        get_available_vram_bytes_for_devices(&selected_gpu_device_ids)
            .or_else(get_available_vram_bytes)
    } else {
        get_available_vram_bytes()
    };
    let supports_gpu_offload = shared_backend()?.supports_gpu_offload();
    let sidecar_vram_reserve_bytes = if supports_gpu_offload {
        let mmproj_reserve = llama_mmproj_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|meta| meta.len())
            .unwrap_or(0);
        let mtp_reserve = if llama_mtp_enabled == Some(true) {
            llama_mtp_model_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .and_then(|path| std::fs::metadata(path).ok())
                .map(|meta| meta.len())
                .unwrap_or(0)
        } else {
            0
        };
        mmproj_reserve.saturating_add(mtp_reserve)
    } else {
        0
    };
    let kv_placement_offload_kqv: Option<bool> = if multi_gpu_active {
        match llama_kv_placement.as_deref() {
            Some("split") | Some("pin") => Some(true),
            Some("systemRam") => Some(false),
            _ => None,
        }
    } else {
        None
    };
    let resolved_offload_kqv = if let Some(placement) = kv_placement_offload_kqv {
        Some(placement)
    } else if llama_offload_kqv.is_some() {
        llama_offload_kqv
    } else if !supports_gpu_offload || using_rocm_backend() {
        Some(false)
    } else {
        None
    };
    let resolved_gpu_layers = if let Some(requested) = llama_gpu_layers {
        if supports_gpu_offload {
            requested.min(metadata.layer_count.max(1))
        } else {
            0
        }
    } else if !supports_gpu_offload {
        0
    } else {
        let flash_attention_policy = if using_rocm_backend() {
            llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_DISABLED
        } else {
            llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_AUTO
        };
        plan_smart_gpu_offload(
            &model_path,
            available_memory_bytes,
            available_vram_bytes,
            None,
            512,
            resolved_offload_kqv,
            llama_kv_type.as_deref(),
            flash_attention_policy,
            sidecar_vram_reserve_bytes,
        )?
        .estimated_gpu_layers
    };
    let recommended_context_length = if resolved_gpu_layers == 0 || !supports_gpu_offload {
        compute_cpu_safe_recommended_context_for_metadata(
            &metadata,
            available_memory_bytes,
            llama_kv_type.as_deref(),
            None,
        )
    } else {
        compute_recommended_context_for_gpu_layers(
            &metadata,
            available_memory_bytes,
            available_vram_bytes,
            resolved_gpu_layers,
            resolved_offload_kqv,
            llama_kv_type.as_deref(),
            sidecar_vram_reserve_bytes,
        )
    };

    let (per_device_vram, estimated_placement) = if multi_gpu_active && supports_gpu_offload {
        let per_dev = get_per_device_free_vram(&selected_gpu_device_ids);
        let device_free_aligned: Vec<u64> = selected_gpu_device_ids
            .iter()
            .map(|id| {
                per_dev
                    .iter()
                    .find(|(dev, _, _)| dev == id)
                    .map(|(_, free, _)| *free)
                    .unwrap_or(0)
            })
            .collect();
        let dist_mode = llama_gpu_distribution_mode.as_deref().unwrap_or("balanced");
        let dist = if dist_mode == "manual" {
            let manual_aligned: Vec<u32> = selected_gpu_device_ids
                .iter()
                .map(|id| {
                    llama_gpu_manual_layers
                        .as_ref()
                        .and_then(|m| m.iter().find(|a| a.device_id == *id))
                        .map(|a| a.layers)
                        .unwrap_or(0)
                })
                .collect();
            plan_multi_gpu_distribution(
                "manual",
                &device_free_aligned,
                metadata.layer_count.max(1),
                0,
                0,
                Some(&manual_aligned),
                None,
            )
        } else {
            let flash_attention_policy = if using_rocm_backend() {
                llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_DISABLED
            } else {
                llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_AUTO
            };
            let plan = plan_smart_gpu_offload(
                &model_path,
                available_memory_bytes,
                available_vram_bytes,
                None,
                512,
                resolved_offload_kqv,
                llama_kv_type.as_deref(),
                flash_attention_policy,
                sidecar_vram_reserve_bytes,
            )?;
            plan_multi_gpu_distribution(
                dist_mode,
                &device_free_aligned,
                plan.total_layers,
                plan.bytes_per_layer,
                plan.estimated_gpu_layers,
                None,
                llama_priority_vram_limit_bytes,
            )
        };
        let vram = per_dev
            .into_iter()
            .map(|(index, free, total)| PerDeviceVram {
                index,
                memory_free: free,
                memory_total: total,
            })
            .collect();
        (
            Some(vram),
            Some(EstimatedPlacement {
                total_gpu_layers: dist.n_gpu_layers,
                per_device_layers: dist.per_device_layers,
            }),
        )
    } else {
        (None, None)
    };

    Ok(LlamaCppContextInfo {
        max_context_length: max_ctx,
        recommended_context_length,
        available_memory_bytes,
        available_vram_bytes,
        model_size_bytes: Some(metadata.model_size_bytes),
        layer_count: Some(metadata.layer_count),
        supports_gpu_offload: Some(supports_gpu_offload),
        selected_gpu_device_ids: if multi_gpu_active {
            Some(selected_gpu_device_ids)
        } else {
            None
        },
        per_device_vram,
        estimated_placement,
    })
}
