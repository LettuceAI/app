//! The model editor's fit estimate for a local model: context limits, the
//! machine's memory, how many layers the GPU would take and, with several
//! GPUs, how they would be split. It plans with an unset context and a batch
//! of 512. The draft model reserve counts the DFlash drafter when DFlash is
//! enabled, else the MTP draft model.

use std::path::Path;

use crate::context_preview::{
    MeasuredCompute, PreviewDevice, PreviewHardware, PreviewRequest, PreviewSidecars,
    effective_available_vram, preview_context,
};
use crate::engine::using_rocm_backend;
use crate::hardware::{
    get_available_memory_bytes, get_available_vram_bytes, get_total_memory_bytes, list_gpu_devices,
};
use crate::llama::{
    LlamaRuntimeError, ProjectionShape, estimate_mtp_gpu_reserve_bytes, load_kv_geometry,
    load_model_metadata, load_offload_costs, project_memory, shared_backend,
};
use crate::mtp::model_has_mtp;
use crate::offload::{FlashAttentionPolicy, KvCacheTypes, OffloadModel, drafter_reserve_path};
use crate::request::{normalized_mtp_placement, resolve_kv_offload};

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

fn mmproj_reserve_bytes(path: Option<&str>) -> u64 {
    path.map(str::trim)
        .filter(|path| !path.is_empty())
        .and_then(|path| std::fs::metadata(path).ok())
        .map_or(0, |meta| meta.len())
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
    let geometry = load_kv_geometry(&model_path);
    let supports_gpu_offload = shared_backend()?.supports_gpu_offload();
    let kv_types = KvCacheTypes::from_settings(
        request.llama_kv_type.as_deref(),
        request.llama_kv_type_k.as_deref(),
        request.llama_kv_type_v.as_deref(),
    );
    let flash_policy = request
        .flash_attention
        .unwrap_or_else(flash_attention_policy);
    let resolved_offload_kqv = resolve_kv_offload(
        request.llama_kv_placement.as_deref(),
        request.llama_offload_kqv,
    )
    .or_else(|| (!supports_gpu_offload || using_rocm_backend()).then_some(false));
    let options = PreviewRequest {
        context_length: request.context_length,
        batch_size: request.batch_size,
        flash_attention: Some(flash_policy),
        swa_full: request.swa_full,
        llama_offload_kqv: resolved_offload_kqv,
        llama_kv_type: request.llama_kv_type.clone(),
        llama_kv_type_k: request.llama_kv_type_k.clone(),
        llama_kv_type_v: request.llama_kv_type_v.clone(),
        llama_gpu_layers: request.llama_gpu_layers,
        llama_multi_gpu_enabled: request.llama_multi_gpu_enabled,
        llama_gpu_device_ids: request.llama_gpu_device_ids,
        llama_gpu_distribution_mode: request.llama_gpu_distribution_mode,
        llama_gpu_manual_layers: request.llama_gpu_manual_layers,
        llama_single_gpu_device_id: request.llama_single_gpu_device_id,
        llama_kv_placement: request.llama_kv_placement,
        llama_priority_vram_limit_bytes: request.llama_priority_vram_limit_bytes,
    };
    let hardware = PreviewHardware {
        available_memory_bytes: get_available_memory_bytes(),
        total_memory_bytes: get_total_memory_bytes(),
        available_vram_bytes: get_available_vram_bytes(),
        supports_gpu_offload,
        unified_memory: crate::hardware::is_unified_memory(),
        devices: list_gpu_devices()
            .into_iter()
            .map(|device| PreviewDevice {
                index: device.index,
                name: device.name,
                memory_free: device.memory_free,
                memory_total: device.memory_total,
                integrated: device.device_type == "IntegratedGpu",
            })
            .collect(),
    };
    let available_vram = effective_available_vram(&options, &hardware);
    let mtp_placement = normalized_mtp_placement(request.llama_mtp_placement.as_deref());
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
    let mmproj_reserve = mmproj_reserve_bytes(request.llama_mmproj_path.as_deref());
    let n_batch = request.batch_size.unwrap_or(512).max(1);
    let probe = |gpu_layers: u32, context: u32| {
        project_memory(
            &model_path,
            ProjectionShape {
                n_gpu_layers: gpu_layers,
                planned_context: context,
                n_batch,
                offload_kqv: resolved_offload_kqv,
                kv_types,
                flash_attention_policy: flash_policy,
                swa_full: request.swa_full,
            },
        )
        .map(|projection| MeasuredCompute {
            device_bytes: projection.device_compute,
            host_bytes: projection.host_compute,
        })
    };
    Ok(preview_context(
        OffloadModel {
            metadata,
            costs: costs.as_ref(),
            geometry: geometry.as_ref(),
        },
        &options,
        &hardware,
        PreviewSidecars {
            vram_reserve_bytes: if supports_gpu_offload {
                mmproj_reserve.saturating_add(mtp_reserve)
            } else {
                0
            },
            mtp_vram_reserve_bytes: mtp_reserve,
            bundled_mtp_draft: request.llama_mtp_enabled == Some(true)
                && model_has_mtp(&model_path),
        },
        Some(&probe),
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
    fn the_projector_path_is_trimmed_like_the_runtime_trims_it() {
        let dir = std::env::temp_dir().join(format!("lettuce-mmproj-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("mmproj.gguf");
        std::fs::write(&file, [0u8; 64]).expect("file");
        let padded = format!("  {}\n", file.display());
        assert_eq!(mmproj_reserve_bytes(Some(&padded)), 64);
        assert_eq!(mmproj_reserve_bytes(Some("   ")), 0);
        assert_eq!(mmproj_reserve_bytes(None), 0);
        std::fs::remove_dir_all(dir).expect("cleanup");
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
