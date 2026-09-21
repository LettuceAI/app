//! The model editor's fit estimate for a local model: context limits, the
//! machine's memory, how many layers the GPU would take and, with several
//! GPUs, how they would be split. It plans with an unset context and a batch
//! of 512, as legacy did.

use std::path::Path;

use crate::context::{
    combined_effective_vram_bytes, compute_cpu_safe_recommended_context_for_metadata,
};
use crate::engine::using_rocm_backend;
use crate::hardware::{
    get_aligned_per_device_vram, get_available_memory_bytes, get_available_vram_bytes,
};
use crate::llama::{
    LlamaRuntimeError, estimate_mtp_gpu_reserve_bytes, load_kv_geometry, load_model_metadata,
    load_offload_costs, plan_smart_gpu_offload, shared_backend,
};
use crate::mtp::model_has_mtp;
use crate::offload::{
    FlashAttentionPolicy, OffloadRequest, compute_recommended_context_for_gpu_layers,
    plan_multi_gpu_distribution, reserve_device_vram, select_mtp_gpu_device,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuLayerAssignment {
    pub device_id: usize,
    pub layers: u32,
}

#[derive(Clone, Debug, Default)]
pub struct ContextInfoRequest {
    pub model_path: String,
    pub llama_offload_kqv: Option<bool>,
    pub llama_kv_type: Option<String>,
    pub llama_gpu_layers: Option<u32>,
    pub llama_multi_gpu_enabled: Option<bool>,
    pub llama_gpu_device_ids: Option<Vec<usize>>,
    pub llama_gpu_distribution_mode: Option<String>,
    pub llama_gpu_manual_layers: Option<Vec<GpuLayerAssignment>>,
    pub llama_single_gpu_device_id: Option<usize>,
    pub llama_kv_placement: Option<String>,
    pub llama_priority_vram_limit_bytes: Option<u64>,
    pub llama_mmproj_path: Option<String>,
    pub llama_mtp_enabled: Option<bool>,
    pub llama_mtp_placement: Option<String>,
    pub llama_mtp_model_path: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PerDeviceVram {
    pub index: usize,
    pub memory_free: u64,
    pub memory_total: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EstimatedPlacement {
    pub total_gpu_layers: u32,
    pub per_device_layers: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlamaCppContextInfo {
    pub max_context_length: u32,
    pub recommended_context_length: Option<u32>,
    pub available_memory_bytes: Option<u64>,
    pub available_vram_bytes: Option<u64>,
    pub model_size_bytes: Option<u64>,
    pub layer_count: Option<u32>,
    pub max_gpu_layers: Option<u32>,
    pub supports_gpu_offload: Option<bool>,
    pub selected_gpu_device_ids: Option<Vec<usize>>,
    pub per_device_vram: Option<Vec<PerDeviceVram>>,
    pub estimated_placement: Option<EstimatedPlacement>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContextInfoError {
    #[error("llama.cpp model path is empty")]
    EmptyPath,
    #[error("llama.cpp model path not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    Runtime(#[from] LlamaRuntimeError),
}

fn flash_attention_policy() -> FlashAttentionPolicy {
    if using_rocm_backend() {
        FlashAttentionPolicy::Disabled
    } else {
        FlashAttentionPolicy::Auto
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the legacy estimate, kept in one place"
)]
pub fn context_info(request: ContextInfoRequest) -> Result<LlamaCppContextInfo, ContextInfoError> {
    let model_path = request.model_path;
    if model_path.trim().is_empty() {
        return Err(ContextInfoError::EmptyPath);
    }
    if !Path::new(&model_path).exists() {
        return Err(ContextInfoError::NotFound(model_path));
    }
    let llama_kv_type = request.llama_kv_type.as_deref();

    let metadata = load_model_metadata(&model_path)?;
    let max_ctx = metadata.max_context_length.max(1);
    let available_memory_bytes = get_available_memory_bytes();
    let selected_gpu_device_ids = request.llama_gpu_device_ids.unwrap_or_default();
    let multi_gpu_active = request.llama_multi_gpu_enabled == Some(true)
        && selected_gpu_device_ids.len() >= 2
        && request.llama_single_gpu_device_id.is_none();
    let aligned_per_device_vram = if multi_gpu_active {
        get_aligned_per_device_vram(&selected_gpu_device_ids)
    } else {
        Vec::new()
    };
    let available_vram_bytes = if multi_gpu_active {
        combined_effective_vram_bytes(&aligned_per_device_vram).or_else(get_available_vram_bytes)
    } else if let Some(device_id) = request.llama_single_gpu_device_id {
        get_aligned_per_device_vram(&[device_id])
            .first()
            .map(|(_, free, _)| *free)
            .filter(|free| *free > 0)
            .or_else(get_available_vram_bytes)
    } else {
        get_available_vram_bytes()
    };
    let supports_gpu_offload = shared_backend()?.supports_gpu_offload();
    let mtp_placement = request
        .llama_mtp_placement
        .as_deref()
        .map_or("auto", str::trim)
        .to_ascii_lowercase();
    let bundled_mtp_draft = request.llama_mtp_enabled == Some(true) && model_has_mtp(&model_path);
    let mtp_requested_reserve_bytes = if supports_gpu_offload
        && request.llama_mtp_enabled == Some(true)
        && mtp_placement != "cpu"
    {
        request
            .llama_mtp_model_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .map(|path| estimate_mtp_gpu_reserve_bytes(path, 16_384, 512, llama_kv_type))
            .transpose()?
            .unwrap_or(0)
    } else {
        0
    };
    let mtp_vram_reserve_bytes = if mtp_placement == "gpu"
        || (mtp_placement == "auto"
            && available_vram_bytes
                .is_some_and(|bytes| mtp_requested_reserve_bytes < bytes.saturating_mul(9) / 10))
    {
        mtp_requested_reserve_bytes
    } else {
        0
    };
    let sidecar_vram_reserve_bytes = if supports_gpu_offload {
        let mmproj_reserve = request
            .llama_mmproj_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .and_then(|path| std::fs::metadata(path).ok())
            .map_or(0, |meta| meta.len());
        mmproj_reserve.saturating_add(mtp_vram_reserve_bytes)
    } else {
        0
    };
    let kv_placement_offload_kqv: Option<bool> = if multi_gpu_active {
        match request.llama_kv_placement.as_deref() {
            Some("split" | "pin") => Some(true),
            Some("systemRam") => Some(false),
            _ => None,
        }
    } else {
        None
    };
    let resolved_offload_kqv = if let Some(placement) = kv_placement_offload_kqv {
        Some(placement)
    } else if request.llama_offload_kqv.is_some() {
        request.llama_offload_kqv
    } else if !supports_gpu_offload || using_rocm_backend() {
        Some(false)
    } else {
        None
    };
    let offload_request = OffloadRequest {
        available_memory_bytes,
        available_vram_bytes,
        requested_context: None,
        n_batch: 512,
        resolved_offload_kqv,
        llama_kv_type,
        flash_attention_policy: flash_attention_policy(),
        sidecar_vram_reserve_bytes,
        bundled_mtp_draft,
    };
    let resolved_gpu_layers = if let Some(requested) = request.llama_gpu_layers {
        if supports_gpu_offload {
            metadata.normalize_requested_gpu_layers(requested)
        } else {
            0
        }
    } else if !supports_gpu_offload {
        0
    } else {
        plan_smart_gpu_offload(&model_path, offload_request)?.estimated_gpu_layers
    };
    let recommended_context_length = if resolved_gpu_layers == 0 || !supports_gpu_offload {
        compute_cpu_safe_recommended_context_for_metadata(
            &metadata,
            available_memory_bytes,
            llama_kv_type,
            None,
        )
    } else {
        compute_recommended_context_for_gpu_layers(
            &metadata,
            load_offload_costs(&model_path).as_ref(),
            load_kv_geometry(&model_path).as_ref(),
            512,
            available_memory_bytes,
            available_vram_bytes,
            resolved_gpu_layers,
            resolved_offload_kqv,
            llama_kv_type,
            sidecar_vram_reserve_bytes,
        )
    };

    let (per_device_vram, estimated_placement) = if multi_gpu_active && supports_gpu_offload {
        let device_free_aligned: Vec<u64> = aligned_per_device_vram
            .iter()
            .map(|(_, free, _)| *free)
            .collect();
        let mtp_device_id = if mtp_vram_reserve_bytes > 0 {
            select_mtp_gpu_device(&selected_gpu_device_ids, &device_free_aligned)
        } else {
            None
        };
        let distribution_device_free = reserve_device_vram(
            &selected_gpu_device_ids,
            &device_free_aligned,
            mtp_device_id,
            mtp_vram_reserve_bytes,
        );
        let dist_mode = request
            .llama_gpu_distribution_mode
            .as_deref()
            .unwrap_or("balanced");
        let distribution = if dist_mode == "manual" {
            let manual_aligned: Vec<u32> = selected_gpu_device_ids
                .iter()
                .map(|id| {
                    request
                        .llama_gpu_manual_layers
                        .as_ref()
                        .and_then(|manual| manual.iter().find(|entry| entry.device_id == *id))
                        .map_or(0, |entry| entry.layers)
                })
                .collect();
            plan_multi_gpu_distribution(
                "manual",
                &distribution_device_free,
                metadata.offload_layer_count(),
                0,
                0,
                0,
                Some(&manual_aligned),
                None,
                None,
            )
        } else {
            let plan = plan_smart_gpu_offload(&model_path, offload_request)?;
            plan_multi_gpu_distribution(
                dist_mode,
                &distribution_device_free,
                plan.total_layers,
                plan.bytes_per_layer,
                plan.kv_bytes_per_layer,
                plan.estimated_gpu_layers,
                None,
                request.llama_priority_vram_limit_bytes,
                Some(&plan.offload_unit_costs),
            )
        };
        (
            Some(
                aligned_per_device_vram
                    .into_iter()
                    .map(|(index, free, total)| PerDeviceVram {
                        index,
                        memory_free: free,
                        memory_total: total,
                    })
                    .collect(),
            ),
            Some(EstimatedPlacement {
                total_gpu_layers: distribution.n_gpu_layers,
                per_device_layers: distribution.per_device_layers,
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
        layer_count: Some(metadata.model_layer_count()),
        max_gpu_layers: Some(metadata.offload_layer_count()),
        supports_gpu_offload: Some(supports_gpu_offload),
        selected_gpu_device_ids: multi_gpu_active.then_some(selected_gpu_device_ids),
        per_device_vram,
        estimated_placement,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_paths_are_refused_before_any_load() {
        assert_eq!(
            context_info(ContextInfoRequest::default()),
            Err(ContextInfoError::EmptyPath)
        );
        assert_eq!(
            context_info(ContextInfoRequest {
                model_path: "/nowhere/model.gguf".into(),
                ..ContextInfoRequest::default()
            }),
            Err(ContextInfoError::NotFound("/nowhere/model.gguf".into()))
        );
    }

    #[test]
    #[ignore = "needs a local GGUF model in LETTUCE_PLAN_MODEL"]
    fn estimates_a_real_model() {
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let info = context_info(ContextInfoRequest {
            model_path: path,
            llama_kv_type: Some("q8_0".into()),
            ..ContextInfoRequest::default()
        })
        .expect("estimate");
        println!("{info:?}");
        assert!(info.max_context_length > 0);
        assert!(info.recommended_context_length.is_some());
    }
}
