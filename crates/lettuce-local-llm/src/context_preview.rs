use crate::context::{
    combined_effective_vram_bytes, compute_cpu_safe_recommended_context_for_metadata,
};
use crate::offload::{
    FlashAttentionPolicy, KvCacheTypes, OffloadModel, OffloadRequest,
    compute_recommended_context_for_gpu_layers, estimate_memory_footprints,
    plan_multi_gpu_distribution, reserve_device_vram, select_mtp_gpu_device,
};

#[derive(Clone, Debug, Default)]
pub struct PreviewRequest {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuLayerAssignment {
    pub device_id: usize,
    pub layers: u32,
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
    pub placement_fits: bool,
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

#[derive(Clone, Debug, Default)]
pub struct PreviewDevice {
    pub index: usize,
    pub name: String,
    pub memory_free: u64,
    pub memory_total: u64,
    pub integrated: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PreviewHardware {
    pub available_memory_bytes: Option<u64>,
    pub total_memory_bytes: u64,
    pub available_vram_bytes: Option<u64>,
    pub supports_gpu_offload: bool,
    pub unified_memory: bool,
    pub devices: Vec<PreviewDevice>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PreviewSidecars {
    pub vram_reserve_bytes: u64,
    pub mtp_vram_reserve_bytes: u64,
    pub bundled_mtp_draft: bool,
}

pub fn preview_context(
    model: OffloadModel<'_>,
    request: &PreviewRequest,
    hardware: &PreviewHardware,
    sidecars: PreviewSidecars,
) -> LlamaCppContextInfo {
    let metadata = model.metadata;
    let costs = model.costs;
    let geometry = model.geometry;
    let max_ctx = metadata.max_context_length.max(1);
    let n_batch = request.batch_size.unwrap_or(512).max(1);
    let flash_policy = request
        .flash_attention
        .unwrap_or(FlashAttentionPolicy::Auto);
    let kv_types = KvCacheTypes::from_settings(
        request.llama_kv_type.as_deref(),
        request.llama_kv_type_k.as_deref(),
        request.llama_kv_type_v.as_deref(),
    );
    let available_memory_bytes = hardware.available_memory_bytes;
    let requested_ids = request.llama_gpu_device_ids.clone().unwrap_or_default();
    let mut selected_gpu_device_ids = Vec::new();
    for id in &requested_ids {
        if !selected_gpu_device_ids.contains(id)
            && hardware
                .devices
                .iter()
                .any(|device| device.index == *id && !device.integrated)
        {
            selected_gpu_device_ids.push(*id);
        }
    }
    let multi_gpu_requested = request.llama_multi_gpu_enabled == Some(true)
        && request.llama_single_gpu_device_id.is_none();
    let multi_gpu_active = multi_gpu_requested && selected_gpu_device_ids.len() >= 2;
    let selection_valid = !multi_gpu_requested
        || multi_gpu_active && selected_gpu_device_ids.len() == requested_ids.len();
    let aligned_per_device_vram = if multi_gpu_active {
        selected_gpu_device_ids
            .iter()
            .filter_map(|id| {
                hardware
                    .devices
                    .iter()
                    .find(|device| device.index == *id)
                    .map(|device| (device.index, device.memory_free, device.memory_total))
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let available_vram_bytes = if multi_gpu_active {
        combined_effective_vram_bytes(&aligned_per_device_vram)
    } else if let Some(id) = request.llama_single_gpu_device_id {
        hardware
            .devices
            .iter()
            .find(|device| device.index == id)
            .map(|device| device.memory_free)
    } else {
        hardware.available_vram_bytes
    };
    let supports_gpu_offload = hardware.supports_gpu_offload;
    let sidecar_vram_reserve_bytes = sidecars.vram_reserve_bytes;
    let mtp_vram_reserve_bytes = sidecars.mtp_vram_reserve_bytes;
    let bundled_mtp_draft = sidecars.bundled_mtp_draft;
    let resolved_offload_kqv = request.llama_offload_kqv;
    let offload_model = OffloadModel {
        metadata,
        costs,
        geometry,
    };
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
        crate::offload::plan_smart_gpu_offload(offload_model, offload_request, |_, _| None)
            .estimated_gpu_layers
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
            costs,
            geometry,
            n_batch,
            available_memory_bytes,
            available_vram_bytes,
            resolved_gpu_layers,
            resolved_offload_kqv,
            kv_types,
            sidecar_vram_reserve_bytes,
        )
    };

    let mut placement_fits = selection_valid;
    let mut multi_working_bytes = None;
    let (per_device_vram, estimated_placement) = if multi_gpu_active
        && supports_gpu_offload
        && request.llama_gpu_layers != Some(0)
    {
        let device_free_aligned: Vec<u64> = aligned_per_device_vram
            .iter()
            .map(|(_, free, _)| *free)
            .collect();
        let mtp_device_id = if mtp_vram_reserve_bytes > 0 {
            select_mtp_gpu_device(&selected_gpu_device_ids, &device_free_aligned)
        } else {
            None
        };
        let mut distribution_device_free = reserve_device_vram(
            &selected_gpu_device_ids,
            &device_free_aligned,
            mtp_device_id,
            mtp_vram_reserve_bytes,
        );
        let plan =
            crate::offload::plan_smart_gpu_offload(offload_model, offload_request, |_, _| None);
        let mut working = 0u64;
        for (free, original) in distribution_device_free
            .iter_mut()
            .zip(&device_free_aligned)
        {
            let reserve = crate::offload::estimated_runtime_reserve_bytes(
                &metadata,
                *original,
                request.context_length.unwrap_or(plan.planned_context),
                n_batch,
                flash_policy,
            );
            working = working.saturating_add(reserve);
            *free = free.saturating_sub(reserve);
        }
        if let Some(first) = distribution_device_free.first_mut() {
            *first = first
                .saturating_sub(sidecar_vram_reserve_bytes.saturating_sub(mtp_vram_reserve_bytes));
        }
        multi_working_bytes = Some(working.saturating_add(sidecar_vram_reserve_bytes));
        let dist_mode = request
            .llama_gpu_distribution_mode
            .as_deref()
            .unwrap_or("balanced");
        let mut distribution = if dist_mode == "manual" {
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
            plan_multi_gpu_distribution(
                dist_mode,
                &distribution_device_free,
                plan.total_layers,
                plan.bytes_per_layer,
                plan.kv_bytes_per_layer,
                request
                    .llama_gpu_layers
                    .map_or(plan.estimated_gpu_layers, |_| resolved_gpu_layers),
                None,
                request.llama_priority_vram_limit_bytes,
                Some(&plan.offload_unit_costs),
            )
        };
        placement_fits &= crate::offload::multi_gpu_placement_fits(
            &plan.offload_unit_costs,
            distribution.n_gpu_layers,
            &distribution.tensor_split,
            &distribution_device_free,
        );
        if request.llama_gpu_layers.is_some() && distribution.n_gpu_layers < resolved_gpu_layers {
            placement_fits = false;
            let count = selected_gpu_device_ids.len() as u32;
            distribution.n_gpu_layers = resolved_gpu_layers;
            distribution.per_device_layers = (0..count)
                .map(|index| {
                    resolved_gpu_layers / count + u32::from(index < resolved_gpu_layers % count)
                })
                .collect();
        }
        (
            Some(
                aligned_per_device_vram
                    .iter()
                    .copied()
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
        (
            None,
            Some(EstimatedPlacement {
                total_gpu_layers: resolved_gpu_layers,
                per_device_layers: vec![resolved_gpu_layers],
            }),
        )
    };

    let gpu_layers = estimated_placement
        .as_ref()
        .map_or(resolved_gpu_layers, |placement| placement.total_gpu_layers);
    let (mut ram, mut gpu) = estimate_memory_footprints(
        OffloadModel {
            metadata,
            costs,
            geometry,
        },
        OffloadRequest {
            requested_context: request.context_length.or(recommended_context_length),
            ..offload_request
        },
        gpu_layers,
    );
    if let Some(working) = multi_working_bytes {
        gpu.working_bytes = working;
    }
    if gpu_layers == 0 {
        ram.working_bytes = ram
            .working_bytes
            .saturating_add(sidecars.vram_reserve_bytes);
    }
    if let Some(available) = available_vram_bytes {
        placement_fits &= gpu.used_bytes() <= available;
    }
    let mut memory_fit = Vec::new();
    if let Some(available) = available_memory_bytes.filter(|_| ram.used_bytes() > 0) {
        memory_fit.push(MemoryFit {
            gpu: false,
            devices: Vec::new(),
            capacity_bytes: hardware.total_memory_bytes,
            available_bytes: available,
            model_bytes: ram.model_bytes,
            context_bytes: ram.context_bytes,
            working_bytes: ram.working_bytes,
        });
    }
    if let Some(available) = available_vram_bytes.filter(|_| gpu.used_bytes() > 0) {
        let devices = &hardware.devices;
        let selected: Vec<_> = if multi_gpu_active {
            selected_gpu_device_ids
                .iter()
                .filter_map(|id| devices.iter().find(|device| device.index == *id))
                .collect()
        } else if let Some(id) = request.llama_single_gpu_device_id {
            devices.iter().filter(|device| device.index == id).collect()
        } else {
            devices
                .iter()
                .max_by_key(|device| device.memory_free)
                .into_iter()
                .collect()
        };
        let capacity = selected
            .iter()
            .fold(0u64, |sum, device| sum.saturating_add(device.memory_total));
        if !selected.is_empty() {
            memory_fit.push(MemoryFit {
                gpu: true,
                devices: selected.iter().map(|device| device.name.clone()).collect(),
                capacity_bytes: capacity,
                available_bytes: available,
                model_bytes: gpu.model_bytes,
                context_bytes: gpu.context_bytes,
                working_bytes: gpu.working_bytes,
            });
        }
    }

    if hardware.unified_memory && !memory_fit.is_empty() {
        let capacity_bytes = hardware.total_memory_bytes;
        let available_bytes = available_memory_bytes.unwrap_or(0);
        let mut shared = MemoryFit {
            gpu: false,
            devices: Vec::new(),
            capacity_bytes,
            available_bytes,
            model_bytes: 0,
            context_bytes: 0,
            working_bytes: 0,
        };
        for pool in memory_fit.drain(..) {
            shared.devices.extend(pool.devices);
            shared.model_bytes = shared.model_bytes.saturating_add(pool.model_bytes);
            shared.context_bytes = shared.context_bytes.saturating_add(pool.context_bytes);
            shared.working_bytes = shared.working_bytes.saturating_add(pool.working_bytes);
        }
        memory_fit.push(shared);
    }

    LlamaCppContextInfo {
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
        memory_fit: if selection_valid {
            memory_fit
        } else {
            Vec::new()
        },
        placement_fits,
    }
}
