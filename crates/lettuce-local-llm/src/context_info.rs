//! The model editor's fit estimate for a local model: context limits, the
//! machine's memory, how many layers the GPU would take and, with several
//! GPUs, how they would be split. It plans with an unset context and a batch
//! of 512. The draft model reserve counts the DFlash drafter when DFlash is
//! enabled, else the MTP draft model.

use std::path::Path;

use crate::engine::using_rocm_backend;
use crate::hardware::{
    get_available_memory_bytes, get_available_vram_bytes, get_total_memory_bytes, list_gpu_devices,
};
use crate::llama::{
    LlamaRuntimeError, estimate_mtp_gpu_reserve_bytes, load_kv_geometry, load_model_metadata,
    load_offload_costs, shared_backend,
};
use crate::mtp::model_has_mtp;
use crate::offload::{FlashAttentionPolicy, KvCacheTypes, OffloadModel, drafter_reserve_path};
use crate::request::resolve_kv_offload;

pub use crate::context_preview::{
    EstimatedPlacement, GpuLayerAssignment, LlamaCppContextInfo, MemoryFit, PerDeviceVram,
};

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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContextInfoError {
    #[error("llama.cpp model path is empty")]
    EmptyPath,
    #[error("llama.cpp model path not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    Runtime(#[from] LlamaRuntimeError),
}

pub fn flash_attention_policy() -> FlashAttentionPolicy {
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
    let metadata = load_model_metadata(&model_path)?;
    let costs = load_offload_costs(&model_path);
    let geometry = load_kv_geometry(&model_path)
        .map(|geometry| geometry.with_swa_full(request.swa_full.unwrap_or(true)));
    let supports_gpu_offload = shared_backend()?.supports_gpu_offload();
    let available_vram = get_available_vram_bytes();
    let kv_types = KvCacheTypes::from_settings(
        request.llama_kv_type.as_deref(),
        request.llama_kv_type_k.as_deref(),
        request.llama_kv_type_v.as_deref(),
    );
    let mtp_placement = request.llama_mtp_placement.as_deref().unwrap_or("auto");
    let drafter = drafter_reserve_path(
        request.llama_dflash_enabled == Some(true),
        request.llama_dflash_model_path.as_deref(),
        request.llama_mtp_enabled == Some(true),
        request.llama_mtp_model_path.as_deref(),
    );
    let requested_reserve = if supports_gpu_offload && mtp_placement != "cpu" {
        drafter
            .map(|path| estimate_mtp_gpu_reserve_bytes(path, 16_384, 512, kv_types))
            .transpose()?
            .unwrap_or(0)
    } else {
        0
    };
    let mtp_reserve = if mtp_placement == "gpu"
        || mtp_placement == "auto"
            && available_vram.is_some_and(|bytes| requested_reserve < bytes.saturating_mul(9) / 10)
    {
        requested_reserve
    } else {
        0
    };
    let mmproj_reserve = request
        .llama_mmproj_path
        .as_deref()
        .and_then(|path| std::fs::metadata(path).ok())
        .map_or(0, |meta| meta.len());
    let options = crate::context_preview::PreviewRequest {
        context_length: request.context_length,
        batch_size: request.batch_size,
        flash_attention: Some(
            request
                .flash_attention
                .unwrap_or_else(flash_attention_policy),
        ),
        swa_full: request.swa_full,
        llama_offload_kqv: resolve_kv_offload(
            request.llama_kv_placement.as_deref(),
            request.llama_offload_kqv,
        )
        .or_else(|| (!supports_gpu_offload || using_rocm_backend()).then_some(false)),
        llama_kv_type: request.llama_kv_type,
        llama_kv_type_k: request.llama_kv_type_k,
        llama_kv_type_v: request.llama_kv_type_v,
        llama_gpu_layers: request.llama_gpu_layers,
        llama_multi_gpu_enabled: request.llama_multi_gpu_enabled,
        llama_gpu_device_ids: request.llama_gpu_device_ids,
        llama_gpu_distribution_mode: request.llama_gpu_distribution_mode,
        llama_gpu_manual_layers: request.llama_gpu_manual_layers,
        llama_single_gpu_device_id: request.llama_single_gpu_device_id,
        llama_kv_placement: request.llama_kv_placement,
        llama_priority_vram_limit_bytes: request.llama_priority_vram_limit_bytes,
    };
    let hardware = crate::context_preview::PreviewHardware {
        available_memory_bytes: get_available_memory_bytes(),
        total_memory_bytes: get_total_memory_bytes(),
        available_vram_bytes: available_vram,
        supports_gpu_offload,
        unified_memory: crate::hardware::is_unified_memory(),
        devices: list_gpu_devices()
            .into_iter()
            .map(|device| crate::context_preview::PreviewDevice {
                index: device.index,
                name: device.name,
                memory_free: device.memory_free,
                memory_total: device.memory_total,
                integrated: device.device_type == "IntegratedGpu",
            })
            .collect(),
    };
    Ok(crate::context_preview::preview_context(
        OffloadModel {
            metadata,
            costs: costs.as_ref(),
            geometry: geometry.as_ref(),
        },
        &options,
        &hardware,
        crate::context_preview::PreviewSidecars {
            vram_reserve_bytes: if supports_gpu_offload {
                mmproj_reserve.saturating_add(mtp_reserve)
            } else {
                0
            },
            mtp_vram_reserve_bytes: mtp_reserve,
            bundled_mtp_draft: request.llama_mtp_enabled == Some(true)
                && model_has_mtp(&model_path),
        },
    ))
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
        let selected = context_info(ContextInfoRequest {
            model_path: path,
            context_length: Some(32768),
            llama_kv_type: Some("q8_0".into()),
            ..ContextInfoRequest::default()
        })
        .expect("selected context estimate");
        assert!(
            selected
                .memory_fit
                .iter()
                .map(|fit| fit.context_bytes)
                .sum::<u64>()
                >= info
                    .memory_fit
                    .iter()
                    .map(|fit| fit.context_bytes)
                    .sum::<u64>()
        );
    }
}
