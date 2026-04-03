use super::engine::shared_backend;
use llama_cpp_2::model::{params::LlamaModelParams, LlamaModel};
use llama_cpp_sys_2::llama_flash_attn_type;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug)]
pub(super) struct LlamaModelMetadata {
    pub(super) model_size_bytes: u64,
    pub(super) layer_count: u32,
    pub(super) max_context_length: u32,
    pub(super) n_embd: u64,
    pub(super) n_head: u64,
    pub(super) n_head_kv: u64,
}

#[derive(Clone, Debug)]
pub(super) struct SmartGpuOffloadPlan {
    pub(super) total_layers: u32,
    pub(super) recommended_context: Option<u32>,
    pub(super) planned_context: u32,
    pub(super) estimated_gpu_layers: u32,
    pub(super) candidate_gpu_layers: Vec<u32>,
    pub(super) kqv_vram_reserved: bool,
    pub(super) estimated_kv_bytes: u64,
    pub(super) estimated_runtime_reserve_bytes: u64,
    pub(super) effective_vram_budget_bytes: u64,
}

static MODEL_METADATA_CACHE: OnceLock<Mutex<HashMap<String, LlamaModelMetadata>>> = OnceLock::new();

fn metadata_cache() -> &'static Mutex<HashMap<String, LlamaModelMetadata>> {
    MODEL_METADATA_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn kv_bytes_per_value(llama_kv_type: Option<&str>) -> f64 {
    match llama_kv_type
        .map(|value| value.trim().to_ascii_lowercase())
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

fn estimate_kv_bytes_per_token(
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

fn ram_budget_for_context(metadata: &LlamaModelMetadata, available_memory_bytes: u64) -> u64 {
    let reserve = default_memory_reserve_bytes(available_memory_bytes);
    available_memory_bytes.saturating_sub(metadata.model_size_bytes.saturating_add(reserve))
}

fn compute_recommended_context(
    metadata: &LlamaModelMetadata,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    llama_offload_kqv: Option<bool>,
    llama_kv_type: Option<&str>,
) -> Option<u32> {
    let available_for_ctx = if llama_offload_kqv == Some(true) {
        let vram = available_vram_bytes?;
        let reserve = default_memory_reserve_bytes(vram);
        vram.saturating_sub(reserve)
    } else {
        let ram = available_memory_bytes?;
        ram_budget_for_context(metadata, ram)
    };
    let kv_bytes_per_token = estimate_kv_bytes_per_token(metadata, llama_kv_type)?;
    if kv_bytes_per_token == 0 {
        return None;
    }
    let mut recommended = available_for_ctx / kv_bytes_per_token;
    if recommended > u64::from(metadata.max_context_length) {
        recommended = u64::from(metadata.max_context_length);
    }
    Some(recommended as u32)
}

fn load_model_metadata_uncached(model_path: &str) -> Result<LlamaModelMetadata, String> {
    let backend = shared_backend()?;
    let model = LlamaModel::load_from_file(
        backend.as_ref(),
        model_path,
        &LlamaModelParams::default().with_n_gpu_layers(0),
    )
    .map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to load llama model metadata for smart offload: {e}"),
        )
    })?;

    Ok(LlamaModelMetadata {
        model_size_bytes: model.size(),
        layer_count: model.n_layer().max(1),
        max_context_length: model.n_ctx_train().max(1),
        n_embd: u64::try_from(model.n_embd()).unwrap_or(0).max(1),
        n_head: u64::try_from(model.n_head()).unwrap_or(1).max(1),
        n_head_kv: u64::try_from(model.n_head_kv())
            .unwrap_or_else(|_| u64::try_from(model.n_head()).unwrap_or(1))
            .max(1),
    })
}

pub(super) fn load_model_metadata(model_path: &str) -> Result<LlamaModelMetadata, String> {
    if let Some(metadata) = metadata_cache()
        .lock()
        .map_err(|_| "llama.cpp metadata cache lock poisoned".to_string())?
        .get(model_path)
        .copied()
    {
        return Ok(metadata);
    }

    let metadata = load_model_metadata_uncached(model_path)?;
    metadata_cache()
        .lock()
        .map_err(|_| "llama.cpp metadata cache lock poisoned".to_string())?
        .insert(model_path.to_string(), metadata);
    Ok(metadata)
}

fn push_unique(out: &mut Vec<u32>, value: u32) {
    if !out.contains(&value) {
        out.push(value);
    }
}

fn estimated_runtime_reserve_bytes(
    available_vram_bytes: u64,
    flash_attention_policy: llama_flash_attn_type,
) -> u64 {
    let base_reserve = (available_vram_bytes / 10).max(256 * 1024 * 1024);
    let flash_reserve = if flash_attention_policy == llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_ENABLED
    {
        (available_vram_bytes / 20).max(128 * 1024 * 1024)
    } else {
        0
    };
    base_reserve.saturating_add(flash_reserve)
}

fn candidate_gpu_layers(total_layers: u32, estimated_gpu_layers: u32) -> Vec<u32> {
    if total_layers == 0 {
        return vec![0];
    }

    let estimate = estimated_gpu_layers.min(total_layers);
    if estimate == 0 {
        return vec![0];
    }

    let mut candidates = Vec::new();
    if estimate.saturating_mul(4) >= total_layers.saturating_mul(3) {
        push_unique(&mut candidates, total_layers);
    } else {
        push_unique(
            &mut candidates,
            (estimate.saturating_add((total_layers - estimate) / 2)).min(total_layers),
        );
    }
    push_unique(&mut candidates, estimate);
    push_unique(&mut candidates, estimate.saturating_mul(3) / 4);
    push_unique(&mut candidates, estimate / 2);
    push_unique(&mut candidates, estimate / 4);
    push_unique(&mut candidates, 0);
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    candidates
}

pub(super) fn plan_smart_gpu_offload(
    model_path: &str,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    requested_context: Option<u32>,
    resolved_offload_kqv: Option<bool>,
    llama_kv_type: Option<&str>,
    flash_attention_policy: llama_flash_attn_type,
) -> Result<SmartGpuOffloadPlan, String> {
    let metadata = load_model_metadata(model_path)?;
    let total_layers = metadata.layer_count.max(1);
    let recommended_context = compute_recommended_context(
        &metadata,
        available_memory_bytes,
        available_vram_bytes,
        resolved_offload_kqv,
        llama_kv_type,
    );
    let planned_context = requested_context
        .or(recommended_context)
        .unwrap_or(metadata.max_context_length)
        .clamp(1, metadata.max_context_length);

    let available_vram = available_vram_bytes.unwrap_or(0);
    let effective_vram_budget_bytes = available_vram.saturating_mul(9) / 10;
    let kqv_vram_reserved = resolved_offload_kqv != Some(false);
    let estimated_kv_bytes = if kqv_vram_reserved {
        estimate_kv_bytes_per_token(&metadata, llama_kv_type)
            .unwrap_or(0)
            .saturating_mul(u64::from(planned_context))
    } else {
        0
    };
    let estimated_runtime_reserve_bytes =
        estimated_runtime_reserve_bytes(available_vram, flash_attention_policy);
    let available_for_layers = effective_vram_budget_bytes
        .saturating_sub(estimated_runtime_reserve_bytes)
        .saturating_sub(estimated_kv_bytes);
    let bytes_per_layer = metadata
        .model_size_bytes
        .checked_add(u64::from(total_layers) - 1)
        .and_then(|bytes| bytes.checked_div(u64::from(total_layers)))
        .unwrap_or(0);
    let estimated_gpu_layers = if available_for_layers == 0 || bytes_per_layer == 0 {
        0
    } else {
        u32::try_from((available_for_layers / bytes_per_layer).min(u64::from(total_layers)))
            .unwrap_or(total_layers)
            .min(total_layers)
    };

    Ok(SmartGpuOffloadPlan {
        total_layers,
        recommended_context,
        planned_context,
        estimated_gpu_layers,
        candidate_gpu_layers: candidate_gpu_layers(total_layers, estimated_gpu_layers),
        kqv_vram_reserved,
        estimated_kv_bytes,
        estimated_runtime_reserve_bytes,
        effective_vram_budget_bytes,
    })
}
