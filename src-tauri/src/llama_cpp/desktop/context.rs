use super::*;
use llama_cpp_sys_2::{
    ggml_backend_dev_count, ggml_backend_dev_get, ggml_backend_dev_memory, ggml_backend_dev_type,
    GGML_BACKEND_DEVICE_TYPE_ACCEL, GGML_BACKEND_DEVICE_TYPE_GPU, GGML_BACKEND_DEVICE_TYPE_IGPU,
};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LlamaCppContextInfo {
    max_context_length: u32,
    recommended_context_length: Option<u32>,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    model_size_bytes: Option<u64>,
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

pub(crate) fn get_available_vram_bytes() -> Option<u64> {
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
    let n_head = u64::try_from(model.n_head()).unwrap_or(1).max(1);
    let n_head_kv = u64::try_from(model.n_head_kv()).unwrap_or(n_head).max(1);
    let gqa_correction = n_head_kv as f64 / n_head as f64;
    let effective_n_embd = (n_embd as f64 * gqa_correction) as u64;
    let bytes_per_value = kv_bytes_per_value(llama_kv_type);
    let bytes = (n_layer as f64) * (effective_n_embd as f64) * 2.0 * bytes_per_value;
    Some(bytes.max(0.0) as u64)
}

pub(super) fn compute_recommended_context(
    model: &LlamaModel,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    max_context_length: u32,
    llama_offload_kqv: Option<bool>,
    llama_kv_type: Option<&str>,
) -> Option<u32> {
    let available_for_ctx = if llama_offload_kqv == Some(true) {
        let vram = available_vram_bytes?;
        let reserve = (vram / 5).max(512 * 1024 * 1024);
        vram.saturating_sub(reserve)
    } else {
        let ram = available_memory_bytes?;
        let model_size = model.size();
        let reserve = (ram / 5).max(512 * 1024 * 1024);
        ram.saturating_sub(model_size.saturating_add(reserve))
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

pub(crate) async fn llamacpp_context_info(
    app: AppHandle,
    model_path: String,
    llama_offload_kqv: Option<bool>,
    llama_kv_type: Option<String>,
) -> Result<LlamaCppContextInfo, String> {
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

    let engine = load_engine(Some(&app), &model_path, None)?;
    let model = engine
        .model
        .as_ref()
        .ok_or_else(|| "llama.cpp model unavailable".to_string())?;
    let max_ctx = model.n_ctx_train().max(1);
    let available_memory_bytes = get_available_memory_bytes();
    let available_vram_bytes = get_available_vram_bytes();
    let recommended_context_length = compute_recommended_context(
        model,
        available_memory_bytes,
        available_vram_bytes,
        max_ctx,
        llama_offload_kqv,
        llama_kv_type.as_deref(),
    );

    Ok(LlamaCppContextInfo {
        max_context_length: max_ctx,
        recommended_context_length,
        available_memory_bytes,
        available_vram_bytes,
        model_size_bytes: Some(model.size()),
    })
}
