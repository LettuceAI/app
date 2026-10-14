//! The model editor's fit estimate for a local model: context limits, the
//! machine's memory, how many layers the GPU would take and, with several
//! GPUs, how they would be split. It plans with an unset context and a batch
//! of 512. The draft model reserve counts the DFlash drafter when DFlash is
//! enabled, else the MTP draft model.

use std::path::Path;

use crate::context::{
    combined_effective_vram_bytes, compute_cpu_safe_recommended_context_for_metadata,
};
use crate::engine::using_rocm_backend;
use crate::hardware::{
    get_aligned_per_device_vram, get_available_memory_bytes, get_available_vram_bytes, get_total_memory_bytes, list_gpu_devices,
};
use crate::llama::{
    LlamaRuntimeError, estimate_mtp_gpu_reserve_bytes, load_kv_geometry, load_model_metadata,
    load_offload_costs, shared_backend,
};
use crate::mtp::model_has_mtp;
use crate::offload::{
    FlashAttentionPolicy, KvCacheTypes, OffloadRequest, compute_recommended_context_for_gpu_layers,
    drafter_reserve_path, plan_multi_gpu_distribution, reserve_device_vram, select_mtp_gpu_device,
    OffloadModel, estimate_memory_footprints,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuLayerAssignment {
    pub device_id: usize,
    pub layers: u32,
}

#[derive(Clone, Debug, Default)]
pub struct ContextInfoRequest {
    pub model_path: String,
    pub context_length: Option<u32>,
    pub batch_size: Option<u32>,
    pub flash_attention: Option<FlashAttentionPolicy>,
    pub swa_full: Option<bool>,
    pub llama_offload_kqv: Option<bool>,
    pub llama_kv_type: Option<String>,
    pub llama_kv_type_k: Option<String>,
    pub llama_kv_type_v: Option<String>,
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
    pub llama_dflash_enabled: Option<bool>,
    pub llama_dflash_model_path: Option<String>,
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
    pub memory_fit: Vec<MemoryFit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryFit {
    pub gpu: bool,
    pub devices: Vec<String>,
    pub capacity_bytes: u64,
    pub available_bytes: u64,
    pub model_bytes: u64,
    pub context_bytes: u64,
    pub working_bytes: u64,
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
    let kv_types = KvCacheTypes::from_settings(
        request.llama_kv_type.as_deref(),
        request.llama_kv_type_k.as_deref(),
        request.llama_kv_type_v.as_deref(),
    );

    let metadata = load_model_metadata(&model_path)?;
    let max_ctx = metadata.max_context_length.max(1);
    let n_batch = request.batch_size.unwrap_or(512).max(1);
    let flash_policy = request.flash_attention.unwrap_or_else(flash_attention_policy);
    let costs = load_offload_costs(&model_path);
    let geometry = load_kv_geometry(&model_path).map(|geometry| geometry.with_swa_full(request.swa_full.unwrap_or(true)));
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
    let drafter_path = drafter_reserve_path(
        request.llama_dflash_enabled == Some(true),
        request.llama_dflash_model_path.as_deref(),
        request.llama_mtp_enabled == Some(true),
        request.llama_mtp_model_path.as_deref(),
    );
    let mtp_requested_reserve_bytes = if supports_gpu_offload && mtp_placement != "cpu" {
        drafter_path
            .map(|path| estimate_mtp_gpu_reserve_bytes(path, 16_384, 512, kv_types))
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
    let offload_model = OffloadModel { metadata, costs: costs.as_ref(), geometry: geometry.as_ref() };
    let offload_request = OffloadRequest {
        available_memory_bytes,
        available_vram_bytes,
        requested_context: request.context_length,
        n_batch,
        resolved_offload_kqv,
        kv_types,
        flash_attention_policy: flash_policy,
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
        crate::offload::plan_smart_gpu_offload(offload_model, offload_request, |_, _| None).estimated_gpu_layers
    };
    let recommended_context_length = if resolved_gpu_layers == 0 || !supports_gpu_offload {
        compute_cpu_safe_recommended_context_for_metadata(
            &metadata,
            available_memory_bytes,
            kv_types,
            None,
        )
    } else {
        compute_recommended_context_for_gpu_layers(
            &metadata,
            costs.as_ref(),
            geometry.as_ref(),
            n_batch,
            available_memory_bytes,
            available_vram_bytes,
            resolved_gpu_layers,
            resolved_offload_kqv,
            kv_types,
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
            let plan = crate::offload::plan_smart_gpu_offload(offload_model, offload_request, |_, _| None);
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
                    .iter().copied()
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

    let gpu_layers = estimated_placement.as_ref().map_or(resolved_gpu_layers, |placement| placement.total_gpu_layers);
    let (ram, gpu) = estimate_memory_footprints(
        OffloadModel { metadata, costs: costs.as_ref(), geometry: geometry.as_ref() },
        OffloadRequest { requested_context: request.context_length.or(recommended_context_length), ..offload_request },
        gpu_layers,
    );
    let mut memory_fit = Vec::new();
    if let Some(available) = available_memory_bytes.filter(|_| ram.used_bytes() > 0) {
        memory_fit.push(MemoryFit { gpu: false, devices: Vec::new(), capacity_bytes: get_total_memory_bytes(), available_bytes: available, model_bytes: ram.model_bytes, context_bytes: ram.context_bytes, working_bytes: ram.working_bytes });
    }
    if let Some(available) = available_vram_bytes.filter(|_| gpu.used_bytes() > 0) {
        let devices = list_gpu_devices();
        let selected: Vec<_> = if multi_gpu_active {
            devices.iter().filter(|device| selected_gpu_device_ids.contains(&device.index)).collect()
        } else if let Some(id) = request.llama_single_gpu_device_id {
            devices.iter().filter(|device| device.index == id).collect()
        } else {
            devices.iter().max_by_key(|device| device.memory_free).into_iter().collect()
        };
        let capacity = selected.iter().fold(0u64, |sum, device| sum.saturating_add(device.memory_total));
        if capacity > 0 {
            memory_fit.push(MemoryFit { gpu: true, devices: selected.iter().map(|device| device.name.clone()).collect(), capacity_bytes: capacity, available_bytes: available, model_bytes: gpu.model_bytes, context_bytes: gpu.context_bytes, working_bytes: gpu.working_bytes });
        }
    }

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
        memory_fit,
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
            model_path: path.clone(),
            llama_kv_type: Some("q8_0".into()),
            context_length: Some(16384),
            ..ContextInfoRequest::default()
        })
        .expect("estimate");
        println!("{info:?}");
        assert!(info.max_context_length > 0);
        assert!(info.recommended_context_length.is_some());
        assert!(!info.memory_fit.is_empty());
        let selected = context_info(ContextInfoRequest { model_path: path, context_length: Some(32768), llama_kv_type: Some("q8_0".into()), ..ContextInfoRequest::default() }).expect("selected context estimate");
        assert!(selected.memory_fit.iter().map(|fit| fit.context_bytes).sum::<u64>() >= info.memory_fit.iter().map(|fit| fit.context_bytes).sum::<u64>());
    }
}
