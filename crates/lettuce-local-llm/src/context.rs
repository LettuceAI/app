//! Context sizing for llama.cpp generations: the (context, batch) fallback
//! ladder, the out-of-memory classifier and its error detail, the
//! effective-VRAM choice between the backend's free memory and the platform
//! cap, per-device VRAM alignment, and the recommended and CPU-safe context
//! limits. These formulas are frozen and deliberately keep their own
//! KV-per-value table (the offload planner has a newer one).

use crate::offload::{KvCacheTypes, LlamaModelMetadata};

/// The loaded model's shape the context formulas read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelShape {
    pub n_layer: u32,
    pub n_layer_nextn: u32,
    pub n_embd: i32,
    pub n_head: u32,
    pub n_head_kv: u32,
    pub size: u64,
}

fn push_unique_u32(out: &mut Vec<u32>, value: u32) {
    if !out.contains(&value) {
        out.push(value);
    }
}

pub fn context_attempt_candidates(
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

pub fn is_likely_context_oom_error(raw_error: &str) -> bool {
    let lower = raw_error.to_ascii_lowercase();
    lower.contains("null reference from llama.cpp")
        || lower.contains("out of memory")
        || lower.contains("oom")
        || lower.contains("alloc")
        || lower.contains("reserve")
        || lower.contains("failed to create")
}

pub fn context_error_detail(
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

pub fn choose_effective_vram_bytes(
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

pub fn align_per_device_vram(
    device_ids: &[usize],
    per_device_vram: &[(usize, u64, u64)],
) -> Vec<(usize, u64, u64)> {
    let impute_capacity = per_device_vram
        .iter()
        .map(|(_, free, total)| (*free).max(*total))
        .min()
        .unwrap_or(0);

    device_ids
        .iter()
        .map(|id| {
            per_device_vram
                .iter()
                .find(|(dev, _, _)| dev == id)
                .copied()
                .unwrap_or((*id, impute_capacity, impute_capacity))
        })
        .collect()
}

pub fn combined_effective_vram_bytes(per_device_vram: &[(usize, u64, u64)]) -> Option<u64> {
    let combined: u64 = per_device_vram
        .iter()
        .map(|(_, free, total)| (*free).max(*total))
        .sum();
    (combined > 0).then_some(combined)
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

fn estimate_kv_bytes_per_token(model: &ModelShape, kv_types: KvCacheTypes<'_>) -> Option<u64> {
    let n_layer = u64::from(model.n_layer);
    let n_embd = u64::try_from(model.n_embd).ok()?;
    let n_head = u64::from(model.n_head).max(1);
    let n_head_kv = u64::from(model.n_head_kv).max(1);
    let gqa_correction = n_head_kv as f64 / n_head as f64;
    let effective_n_embd = (n_embd as f64 * gqa_correction) as u64;
    let bytes = match kv_types.shared() {
        Some(shared) => {
            (n_layer as f64) * (effective_n_embd as f64) * 2.0 * kv_bytes_per_value(shared)
        }
        None => {
            (n_layer as f64)
                * (effective_n_embd as f64)
                * (kv_bytes_per_value(kv_types.k) + kv_bytes_per_value(kv_types.v))
        }
    };
    Some(bytes.max(0.0) as u64)
}

fn estimate_kv_bytes_per_token_from_metadata(
    metadata: &LlamaModelMetadata,
    kv_types: KvCacheTypes<'_>,
) -> Option<u64> {
    let n_layer = u64::from(metadata.layer_count.max(1));
    let n_embd = metadata.n_embd.max(1);
    let n_head = metadata.n_head.max(1);
    let n_head_kv = metadata.n_head_kv.max(1);
    let gqa_correction = n_head_kv as f64 / n_head as f64;
    let effective_n_embd = (n_embd as f64 * gqa_correction) as u64;
    let bytes = match kv_types.shared() {
        Some(shared) => {
            (n_layer as f64) * (effective_n_embd as f64) * 2.0 * kv_bytes_per_value(shared)
        }
        None => {
            (n_layer as f64)
                * (effective_n_embd as f64)
                * (kv_bytes_per_value(kv_types.k) + kv_bytes_per_value(kv_types.v))
        }
    };
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

fn model_offload_layer_count(model: &ModelShape) -> u32 {
    model
        .n_layer
        .max(1)
        .saturating_add(model.n_layer_nextn)
        .saturating_add(1)
}

fn ram_budget_for_context(model: &ModelShape, available_memory_bytes: u64, gpu_layers: u32) -> u64 {
    let total_layers = model_offload_layer_count(model);
    let cpu_layers = total_layers.saturating_sub(gpu_layers);
    let cpu_resident = resident_weight_bytes(model.size, total_layers, cpu_layers);
    let reserve = default_memory_reserve_bytes(available_memory_bytes);
    available_memory_bytes.saturating_sub(cpu_resident.saturating_add(reserve))
}

fn cpu_fallback_headroom_bytes(base_budget: u64, available_memory_bytes: u64) -> u64 {
    let availability_headroom = (available_memory_bytes / 10).max(256 * 1024 * 1024);
    let budget_headroom = (base_budget / 4).max(128 * 1024 * 1024);
    let minimum_usable_budget = (128 * 1024 * 1024).min(base_budget);
    let maximum_headroom = base_budget.saturating_sub(minimum_usable_budget);
    availability_headroom
        .min(maximum_headroom)
        .max(budget_headroom.min(maximum_headroom))
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

pub fn compute_recommended_context(
    model: &ModelShape,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    max_context_length: u32,
    gpu_layers: u32,
    llama_offload_kqv: Option<bool>,
    kv_types: KvCacheTypes<'_>,
) -> Option<u32> {
    let available_for_ctx = if llama_offload_kqv == Some(true) {
        let vram = available_vram_bytes?;
        let gpu_resident =
            resident_weight_bytes(model.size, model_offload_layer_count(model), gpu_layers);
        let reserve = default_memory_reserve_bytes(vram);
        vram.saturating_sub(gpu_resident.saturating_add(reserve))
    } else {
        let ram = available_memory_bytes?;
        ram_budget_for_context(model, ram, gpu_layers)
    };
    let kv_bytes_per_token = estimate_kv_bytes_per_token(model, kv_types)?;
    if kv_bytes_per_token == 0 {
        return None;
    }
    let mut recommended = available_for_ctx / kv_bytes_per_token;
    if recommended > u64::from(max_context_length) {
        recommended = u64::from(max_context_length);
    }
    Some(recommended as u32)
}

pub fn compute_cpu_fallback_limits(
    model: &ModelShape,
    available_memory_bytes: Option<u64>,
    max_context_length: u32,
    gpu_layers: u32,
    kv_types: KvCacheTypes<'_>,
    requested_context: Option<u32>,
    requested_batch_size: u32,
) -> Option<(u32, u32)> {
    let available_memory_bytes = available_memory_bytes?;
    let kv_bytes_per_token = estimate_kv_bytes_per_token(model, kv_types)?;
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

pub fn compute_cpu_safe_recommended_context_for_metadata(
    metadata: &LlamaModelMetadata,
    available_memory_bytes: Option<u64>,
    kv_types: KvCacheTypes<'_>,
    requested_context: Option<u32>,
) -> Option<u32> {
    let available_memory_bytes = available_memory_bytes?;
    let kv_bytes_per_token = estimate_kv_bytes_per_token_from_metadata(metadata, kv_types)?;
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
    use super::{
        align_per_device_vram, choose_effective_vram_bytes, combined_effective_vram_bytes,
        cpu_fallback_headroom_bytes, resident_weight_bytes,
    };

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
    fn resident_weights_include_the_output_offload_slot() {
        assert_eq!(resident_weight_bytes(3_400, 34, 34), 3_400);
        assert_eq!(resident_weight_bytes(3_400, 34, 33), 3_300);
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

    #[test]
    fn combined_effective_vram_prefers_reported_capacity_per_device() {
        let per_device = vec![(0, 10, 12), (1, 14, 0)];

        assert_eq!(combined_effective_vram_bytes(&per_device), Some(26));
    }

    #[test]
    fn combined_effective_vram_budgets_capacity_over_current_free() {
        let gib = 1024_u64 * 1024 * 1024;
        let per_device = vec![(0, 4 * gib, 24 * gib)];

        assert_eq!(combined_effective_vram_bytes(&per_device), Some(24 * gib));
    }

    #[test]
    fn aligned_per_device_vram_imputes_missing_selected_devices() {
        let aligned = align_per_device_vram(&[0, 1], &[(0, 10, 12)]);

        assert_eq!(aligned, vec![(0, 10, 12), (1, 12, 12)]);
    }

    #[test]
    fn aligned_per_device_vram_imputes_unreported_device_from_reported_sibling() {
        let gib = 1024_u64 * 1024 * 1024;
        let aligned = align_per_device_vram(&[0, 1], &[(0, 20 * gib, 24 * gib)]);

        assert_eq!(
            aligned,
            vec![(0, 20 * gib, 24 * gib), (1, 24 * gib, 24 * gib)]
        );
    }

    #[test]
    fn aligned_per_device_vram_imputes_from_smallest_reported_device() {
        let gib = 1024_u64 * 1024 * 1024;
        let aligned = align_per_device_vram(
            &[0, 1, 2],
            &[(0, 20 * gib, 24 * gib), (1, 6 * gib, 8 * gib)],
        );

        assert_eq!(aligned[2], (2, 8 * gib, 8 * gib));
    }

    #[test]
    fn split_kv_types_add_each_half_at_its_own_width() {
        let model = super::ModelShape {
            n_layer: 32,
            n_layer_nextn: 0,
            n_embd: 4096,
            n_head: 32,
            n_head_kv: 8,
            size: 4_000_000_000,
        };
        let shared = |kv_type| {
            super::estimate_kv_bytes_per_token(
                &model,
                crate::offload::KvCacheTypes::uniform(Some(kv_type)),
            )
        };
        assert_eq!(shared("f16"), Some(32 * 1024 * 2 * 2));
        assert_eq!(shared("q8_0"), Some(32 * 1024 * 2));
        let split = crate::offload::KvCacheTypes {
            k: Some("f16"),
            v: Some("q8_0"),
        };
        assert_eq!(
            super::estimate_kv_bytes_per_token(&model, split),
            Some(32 * 1024 * 2 + 32 * 1024)
        );
    }
}
