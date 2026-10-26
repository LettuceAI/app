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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeasuredCompute {
    pub device_bytes: u64,
    pub host_bytes: u64,
}

/// Asks for the compute buffers llama.cpp would allocate for
/// `(gpu_layers, context)`; the caller fixes every other part of the shape.
pub type ComputeProbe<'a> = &'a dyn Fn(u32, u32) -> Option<MeasuredCompute>;

#[derive(Clone, Copy, Debug, Default)]
pub struct PreviewSidecars {
    pub vram_reserve_bytes: u64,
    pub mtp_vram_reserve_bytes: u64,
    pub bundled_mtp_draft: bool,
}

struct DeviceSelection {
    ids: Vec<usize>,
    requested: usize,
    multi_gpu_requested: bool,
}

impl DeviceSelection {
    fn new(request: &PreviewRequest, hardware: &PreviewHardware) -> Self {
        let requested_ids = request.llama_gpu_device_ids.clone().unwrap_or_default();
        let mut ids = Vec::new();
        for id in &requested_ids {
            if !ids.contains(id)
                && hardware
                    .devices
                    .iter()
                    .any(|device| device.index == *id && !device.integrated)
            {
                ids.push(*id);
            }
        }
        Self {
            ids,
            requested: requested_ids.len(),
            multi_gpu_requested: request.llama_multi_gpu_enabled == Some(true)
                && request.llama_single_gpu_device_id.is_none()
                && requested_ids.len() >= 2,
        }
    }

    fn multi_gpu_active(&self) -> bool {
        self.multi_gpu_requested && self.ids.len() >= 2
    }

    fn valid(&self) -> bool {
        !self.multi_gpu_requested || self.multi_gpu_active() && self.ids.len() == self.requested
    }
}

/// The VRAM budget the load plans against: the selected GPUs combined, a
/// single selected GPU, or the largest free GPU memory.
pub fn effective_available_vram(
    request: &PreviewRequest,
    hardware: &PreviewHardware,
) -> Option<u64> {
    let selection = DeviceSelection::new(request, hardware);
    if selection.multi_gpu_active() {
        let aligned = aligned_device_vram(&selection, hardware);
        combined_effective_vram_bytes(&aligned).or(hardware.available_vram_bytes)
    } else if let Some(id) = request.llama_single_gpu_device_id {
        hardware
            .devices
            .iter()
            .find(|device| device.index == id)
            .map(|device| device.memory_free)
            .filter(|free| *free > 0)
            .or(hardware.available_vram_bytes)
    } else {
        hardware.available_vram_bytes
    }
}

fn aligned_device_vram(
    selection: &DeviceSelection,
    hardware: &PreviewHardware,
) -> Vec<(usize, u64, u64)> {
    selection
        .ids
        .iter()
        .filter_map(|id| {
            hardware
                .devices
                .iter()
                .find(|device| device.index == *id)
                .map(|device| (device.index, device.memory_free, device.memory_total))
        })
        .collect()
}

pub fn preview_context(
    model: OffloadModel<'_>,
    request: &PreviewRequest,
    hardware: &PreviewHardware,
    sidecars: PreviewSidecars,
    probe: Option<ComputeProbe<'_>>,
) -> LlamaCppContextInfo {
    let metadata = model.metadata;
    let costs = model.costs;
    let windowed_geometry = model.geometry.map(|geometry| {
        geometry
            .clone()
            .with_swa_full(request.swa_full.unwrap_or(false))
    });
    let geometry = windowed_geometry.as_ref();
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
    let selection = DeviceSelection::new(request, hardware);
    let multi_gpu_active = selection.multi_gpu_active();
    let selection_valid = selection.valid();
    let selected_gpu_device_ids = selection.ids.clone();
    let aligned_per_device_vram = if multi_gpu_active {
        aligned_device_vram(&selection, hardware)
    } else {
        Vec::new()
    };
    let available_vram_bytes = effective_available_vram(request, hardware);
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
        swa_full: request.swa_full.unwrap_or(false),
    };
    let measure_device = |gpu_layers: u32, context: u32| {
        probe
            .and_then(|probe| probe(gpu_layers, context))
            .map(|measured| measured.device_bytes)
            .filter(|bytes| *bytes > 0)
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
        crate::offload::plan_smart_gpu_offload(offload_model, offload_request, measure_device)
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
    let manual_distribution = request.llama_gpu_distribution_mode.as_deref() == Some("manual");
    let (per_device_vram, estimated_placement) = if multi_gpu_active
        && supports_gpu_offload
        && (request.llama_gpu_layers != Some(0) || manual_distribution)
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
        let mtp_reserved_free = reserve_device_vram(
            &selected_gpu_device_ids,
            &device_free_aligned,
            mtp_device_id,
            mtp_vram_reserve_bytes,
        );
        let plan =
            crate::offload::plan_smart_gpu_offload(offload_model, offload_request, measure_device);
        let reserve_context = request.context_length.unwrap_or(plan.planned_context);
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
                &device_free_aligned,
                metadata.offload_layer_count(),
                0,
                0,
                0,
                Some(&manual_aligned),
                None,
                None,
            )
        } else if request.llama_gpu_layers.is_some() {
            plan_multi_gpu_distribution(
                dist_mode,
                &device_free_aligned,
                metadata.offload_layer_count(),
                0,
                0,
                resolved_gpu_layers,
                None,
                request.llama_priority_vram_limit_bytes,
                None,
            )
        } else {
            plan_multi_gpu_distribution(
                dist_mode,
                &mtp_reserved_free,
                plan.total_layers,
                plan.bytes_per_layer,
                plan.kv_bytes_per_layer,
                plan.estimated_gpu_layers,
                None,
                request.llama_priority_vram_limit_bytes,
                Some(&plan.offload_unit_costs),
            )
        };
        let measured_reserve = (distribution.n_gpu_layers > 0)
            .then(|| measure_device(distribution.n_gpu_layers, reserve_context))
            .flatten();
        let mut fit_device_free = mtp_reserved_free;
        let mut working = 0u64;
        for (free, original) in fit_device_free.iter_mut().zip(&device_free_aligned) {
            let reserve = match measured_reserve {
                Some(total) => total.div_ceil(device_free_aligned.len() as u64),
                None => crate::offload::estimated_runtime_reserve_bytes(
                    &metadata,
                    *original,
                    reserve_context,
                    n_batch,
                    flash_policy,
                ),
            };
            working = working.saturating_add(reserve);
            *free = free.saturating_sub(reserve);
        }
        if let Some(first) = fit_device_free.first_mut() {
            *first = first
                .saturating_sub(sidecar_vram_reserve_bytes.saturating_sub(mtp_vram_reserve_bytes));
        }
        multi_working_bytes = Some(working.saturating_add(sidecar_vram_reserve_bytes));
        placement_fits &= crate::offload::multi_gpu_placement_fits(
            &plan.offload_unit_costs,
            distribution.n_gpu_layers,
            &distribution.tensor_split,
            &fit_device_free,
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
    let footprint_context = request.context_length.or(recommended_context_length);
    let (mut ram, mut gpu) = estimate_memory_footprints(
        OffloadModel {
            metadata,
            costs,
            geometry,
        },
        OffloadRequest {
            requested_context: footprint_context,
            ..offload_request
        },
        gpu_layers,
    );
    if let Some(working) = multi_working_bytes {
        gpu.working_bytes = working;
    }
    let measured_context = footprint_context
        .unwrap_or(metadata.max_context_length)
        .clamp(1, max_ctx);
    let measured = probe.and_then(|probe| probe(gpu_layers, measured_context));
    if let Some(measured) = measured {
        if gpu_layers > 0 && measured.device_bytes > 0 {
            gpu.working_bytes = measured
                .device_bytes
                .saturating_add(sidecar_vram_reserve_bytes);
        }
        if ram.working_bytes > 0 && measured.host_bytes > 0 {
            ram.working_bytes = measured.host_bytes;
        }
    }
    if gpu_layers == 0 && !supports_gpu_offload {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offload::{
        KvCacheGeometry, KvLayerGeometry, LlamaModelMetadata, ModelOffloadCosts, OffloadRequest,
        plan_smart_gpu_offload,
    };

    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * MIB;

    fn fixture() -> (LlamaModelMetadata, ModelOffloadCosts, KvCacheGeometry) {
        let mut tensors = (0..8)
            .map(|index| (format!("blk.{index}.ffn_up.weight"), 512 * MIB))
            .collect::<Vec<_>>();
        tensors.push(("output.weight".to_owned(), 256 * MIB));
        tensors.push(("token_embd.weight".to_owned(), 256 * MIB));
        let costs = ModelOffloadCosts::from_tensors(
            tensors.iter().map(|(name, bytes)| (name.as_str(), *bytes)),
        )
        .expect("costs");
        let metadata = LlamaModelMetadata {
            model_size_bytes: 4608 * MIB,
            layer_count: 8,
            nextn_layer_count: 0,
            max_context_length: 32768,
            n_embd: 4096,
            n_head: 32,
            n_head_kv: 8,
            n_embd_head_k: 128,
            n_embd_head_v: 128,
        };
        let layers = (0..8)
            .map(|index| KvLayerGeometry {
                n_head_kv: 8,
                n_embd_head_k: 128,
                n_embd_head_v: 128,
                is_swa: index % 2 == 0,
            })
            .collect();
        (
            metadata,
            costs,
            KvCacheGeometry::new(layers, 512).expect("geometry"),
        )
    }

    fn device(index: usize, name: &str, free: u64, total: u64, integrated: bool) -> PreviewDevice {
        PreviewDevice {
            index,
            name: name.to_owned(),
            memory_free: free,
            memory_total: total,
            integrated,
        }
    }

    fn hardware() -> PreviewHardware {
        PreviewHardware {
            available_memory_bytes: Some(24 * GIB),
            total_memory_bytes: 32 * GIB,
            available_vram_bytes: Some(6 * GIB),
            supports_gpu_offload: true,
            unified_memory: false,
            devices: vec![
                device(0, "big", 6 * GIB, 8 * GIB, false),
                device(1, "small", 3 * GIB, 4 * GIB, false),
                device(2, "igpu", 2 * GIB, 2 * GIB, true),
            ],
        }
    }

    fn request() -> PreviewRequest {
        PreviewRequest {
            context_length: Some(4096),
            llama_kv_type: Some("f16".into()),
            llama_offload_kqv: Some(true),
            flash_attention: Some(FlashAttentionPolicy::Auto),
            ..PreviewRequest::default()
        }
    }

    fn multi(ids: &[usize]) -> PreviewRequest {
        PreviewRequest {
            llama_multi_gpu_enabled: Some(true),
            llama_gpu_device_ids: Some(ids.to_vec()),
            ..request()
        }
    }

    fn preview(
        request: &PreviewRequest,
        hardware: &PreviewHardware,
        sidecars: PreviewSidecars,
        probe: Option<ComputeProbe<'_>>,
    ) -> LlamaCppContextInfo {
        let (metadata, costs, geometry) = fixture();
        preview_context(
            OffloadModel {
                metadata,
                costs: Some(&costs),
                geometry: Some(&geometry),
            },
            request,
            hardware,
            sidecars,
            probe,
        )
    }

    fn run(request: &PreviewRequest, hardware: &PreviewHardware) -> LlamaCppContextInfo {
        preview(request, hardware, PreviewSidecars::default(), None)
    }

    fn gpu_pool(info: &LlamaCppContextInfo) -> Option<&MemoryFit> {
        info.memory_fit.iter().find(|pool| pool.gpu)
    }

    fn placed_layers(info: &LlamaCppContextInfo) -> u32 {
        info.estimated_placement
            .as_ref()
            .expect("placement")
            .total_gpu_layers
    }

    fn smart_plan(vram: u64, sidecar: u64) -> crate::offload::SmartGpuOffloadPlan {
        let (metadata, costs, geometry) = fixture();
        plan_smart_gpu_offload(
            OffloadModel {
                metadata,
                costs: Some(&costs),
                geometry: Some(&geometry),
            },
            OffloadRequest {
                available_memory_bytes: Some(24 * GIB),
                available_vram_bytes: Some(vram),
                requested_context: Some(4096),
                n_batch: 512,
                resolved_offload_kqv: Some(true),
                kv_types: KvCacheTypes::uniform(Some("f16")),
                flash_attention_policy: FlashAttentionPolicy::Auto,
                sidecar_vram_reserve_bytes: sidecar,
                bundled_mtp_draft: false,
                swa_full: false,
            },
            |_, _| None,
        )
    }

    #[test]
    fn a_single_device_without_free_memory_falls_back_to_the_machine_budget() {
        let mut machine = hardware();
        let one = PreviewRequest {
            llama_single_gpu_device_id: Some(1),
            ..request()
        };
        assert_eq!(effective_available_vram(&one, &machine), Some(3 * GIB));
        assert_eq!(run(&one, &machine).available_vram_bytes, Some(3 * GIB));
        machine.devices[1].memory_free = 0;
        assert_eq!(effective_available_vram(&one, &machine), Some(6 * GIB));
        let missing = PreviewRequest {
            llama_single_gpu_device_id: Some(9),
            ..request()
        };
        assert_eq!(effective_available_vram(&missing, &machine), Some(6 * GIB));
    }

    #[test]
    fn selected_gpus_combine_their_capacity_in_the_requested_order() {
        let info = run(&multi(&[1, 0]), &hardware());
        assert_eq!(info.selected_gpu_device_ids, Some(vec![1, 0]));
        assert_eq!(info.available_vram_bytes, Some(12 * GIB));
        assert_eq!(
            gpu_pool(&info).expect("gpu pool").devices,
            vec!["small".to_owned(), "big".to_owned()]
        );
        assert!(info.placement_fits);
    }

    #[test]
    fn a_selection_with_a_missing_integrated_or_repeated_device_is_refused() {
        for ids in [[0, 9], [0, 2], [0, 0]] {
            let info = run(&multi(&ids), &hardware());
            assert!(info.memory_fit.is_empty(), "{ids:?}");
            assert!(!info.placement_fits, "{ids:?}");
            assert_eq!(info.selected_gpu_device_ids, None, "{ids:?}");
        }
    }

    #[test]
    fn multi_gpu_with_fewer_than_two_ids_plans_like_the_runtime_does() {
        let info = run(&multi(&[1]), &hardware());
        assert_eq!(info.selected_gpu_device_ids, None);
        assert!(!info.memory_fit.is_empty());
        assert!(info.placement_fits);
        assert_eq!(info.available_vram_bytes, Some(6 * GIB));
    }

    #[test]
    fn zero_layers_leave_nothing_to_distribute() {
        let info = run(
            &PreviewRequest {
                llama_gpu_layers: Some(0),
                ..multi(&[0, 1])
            },
            &hardware(),
        );
        assert_eq!(placed_layers(&info), 0);
        assert_eq!(info.per_device_vram, None);
        assert!(gpu_pool(&info).is_none());
    }

    #[test]
    fn explicit_layers_set_the_multi_gpu_placement() {
        let smart = run(&multi(&[0, 1]), &hardware());
        assert_eq!(placed_layers(&smart), 9);
        let explicit = run(
            &PreviewRequest {
                llama_gpu_layers: Some(4),
                ..multi(&[0, 1])
            },
            &hardware(),
        );
        let placement = explicit.estimated_placement.expect("placement");
        assert_eq!(placement.total_gpu_layers, 4);
        assert_eq!(placement.per_device_layers.iter().sum::<u32>(), 4);
    }

    #[test]
    fn the_distribution_uses_the_free_memory_the_runtime_hands_it() {
        let mut machine = hardware();
        machine.devices[0].memory_free = 2 * GIB + 900 * MIB;
        machine.devices[0].memory_total = 2 * GIB + 900 * MIB;
        machine.devices[1].memory_free = GIB + 300 * MIB;
        machine.devices[1].memory_total = GIB + 300 * MIB;
        let free = [
            machine.devices[0].memory_free,
            machine.devices[1].memory_free,
        ];
        let plan = smart_plan(free[0] + free[1], 0);
        let distribute = |free: &[u64]| {
            plan_multi_gpu_distribution(
                "balanced",
                free,
                plan.total_layers,
                plan.bytes_per_layer,
                plan.kv_bytes_per_layer,
                plan.estimated_gpu_layers,
                None,
                None,
                Some(&plan.offload_unit_costs),
            )
        };
        let runtime = distribute(&free);
        let reserved = distribute(&free.map(|free| free - 256 * MIB));
        assert_ne!(runtime.per_device_layers, reserved.per_device_layers);
        let info = run(&multi(&[0, 1]), &machine);
        assert_eq!(
            info.estimated_placement
                .expect("placement")
                .per_device_layers,
            runtime.per_device_layers
        );
    }

    #[test]
    fn explicit_layers_are_spread_the_way_the_load_spreads_them() {
        let mut machine = hardware();
        machine.devices[0].memory_free = 2 * GIB + 900 * MIB;
        machine.devices[0].memory_total = 2 * GIB + 900 * MIB;
        machine.devices[1].memory_free = GIB + 900 * MIB;
        machine.devices[1].memory_total = GIB + 900 * MIB;
        let free = [
            machine.devices[0].memory_free,
            machine.devices[1].memory_free,
        ];
        let spread = |mode: &str| {
            plan_multi_gpu_distribution(mode, &free, 9, 0, 0, 9, None, None, None).per_device_layers
        };
        let placed = |mode: &str, layers| {
            run(
                &PreviewRequest {
                    llama_gpu_layers: layers,
                    llama_gpu_distribution_mode: Some(mode.into()),
                    ..multi(&[0, 1])
                },
                &machine,
            )
            .estimated_placement
            .expect("placement")
            .per_device_layers
        };
        assert_eq!(placed("balanced", Some(u32::MAX)), spread("balanced"));
        assert_eq!(placed("balanced", Some(u32::MAX)), vec![5, 4]);
        assert_eq!(
            placed("proportional", Some(u32::MAX)),
            spread("proportional")
        );
        assert_ne!(placed("balanced", None), spread("balanced"));
    }

    #[test]
    fn manual_distribution_decides_the_placement_even_when_zero_layers_are_set() {
        let info = run(
            &PreviewRequest {
                llama_gpu_layers: Some(0),
                llama_gpu_distribution_mode: Some("manual".into()),
                llama_gpu_manual_layers: Some(vec![
                    GpuLayerAssignment {
                        device_id: 0,
                        layers: 3,
                    },
                    GpuLayerAssignment {
                        device_id: 1,
                        layers: 2,
                    },
                ]),
                ..multi(&[0, 1])
            },
            &hardware(),
        );
        let placement = info.estimated_placement.expect("placement");
        assert_eq!(placement.per_device_layers, vec![3, 2]);
        assert_eq!(placement.total_gpu_layers, 5);
    }

    #[test]
    fn a_placement_that_only_fits_without_compute_buffers_is_not_a_fit() {
        let tight = |first: u64, second: u64| {
            let mut machine = hardware();
            machine.devices[0].memory_free = first;
            machine.devices[0].memory_total = first;
            machine.devices[1].memory_free = second;
            machine.devices[1].memory_total = second;
            run(
                &PreviewRequest {
                    llama_gpu_layers: Some(u32::MAX),
                    llama_gpu_distribution_mode: Some("proportional".into()),
                    ..multi(&[0, 1])
                },
                &machine,
            )
        };
        let bare = tight(3200 * MIB, 2000 * MIB);
        assert_eq!(placed_layers(&bare), 9);
        assert!(!bare.placement_fits);
        let roomy = tight(3600 * MIB, 2400 * MIB);
        assert_eq!(placed_layers(&roomy), 9);
        assert!(roomy.placement_fits);
    }

    #[test]
    fn a_cpu_only_backend_counts_the_projector_in_ram() {
        let sidecars = PreviewSidecars {
            vram_reserve_bytes: 700 * MIB,
            ..PreviewSidecars::default()
        };
        let mut machine = hardware();
        machine.supports_gpu_offload = false;
        let with = preview(&request(), &machine, sidecars, None);
        let without = preview(&request(), &machine, PreviewSidecars::default(), None);
        let ram = |info: &LlamaCppContextInfo| info.memory_fit[0].working_bytes;
        assert_eq!(ram(&with), ram(&without) + 700 * MIB);

        let on_gpu_machine = PreviewRequest {
            llama_gpu_layers: Some(0),
            ..request()
        };
        let with = preview(&on_gpu_machine, &hardware(), sidecars, None);
        let without = preview(
            &on_gpu_machine,
            &hardware(),
            PreviewSidecars::default(),
            None,
        );
        assert_eq!(ram(&with), ram(&without));
    }

    #[test]
    fn a_gpu_pool_is_kept_when_the_device_reports_no_capacity() {
        let mut machine = hardware();
        machine.devices[0].memory_total = 0;
        let info = run(&request(), &machine);
        let pool = gpu_pool(&info).expect("gpu pool");
        assert_eq!(pool.capacity_bytes, 0);
        assert!(pool.model_bytes > 0);
    }

    #[test]
    fn unified_memory_folds_both_pools_into_physical_ram() {
        let separate = run(&request(), &hardware());
        assert_eq!(separate.memory_fit.len(), 2);
        let mut machine = hardware();
        machine.unified_memory = true;
        let shared = run(&request(), &machine);
        assert_eq!(shared.memory_fit.len(), 1);
        let pool = &shared.memory_fit[0];
        assert_eq!(pool.capacity_bytes, 32 * GIB);
        assert_eq!(pool.available_bytes, 24 * GIB);
        let sum = |field: fn(&MemoryFit) -> u64| separate.memory_fit.iter().map(field).sum::<u64>();
        assert_eq!(pool.model_bytes, sum(|fit| fit.model_bytes));
        assert_eq!(pool.context_bytes, sum(|fit| fit.context_bytes));
        assert_eq!(pool.working_bytes, sum(|fit| fit.working_bytes));
    }

    #[test]
    fn sliding_window_caches_are_the_default_and_swa_full_grows_them() {
        let context = |swa_full| {
            let info = run(
                &PreviewRequest {
                    swa_full,
                    ..request()
                },
                &hardware(),
            );
            gpu_pool(&info).expect("gpu pool").context_bytes
        };
        assert_eq!(context(None), context(Some(false)));
        assert!(context(Some(true)) > context(None));
    }

    #[test]
    fn the_preview_and_the_load_plan_agree_for_both_swa_modes() {
        let (metadata, costs, geometry) = fixture();
        let mut differing = 0;
        for step in 8..64u64 {
            let vram = step * 128 * MIB;
            let mut machine = hardware();
            machine.available_vram_bytes = Some(vram);
            machine.devices = vec![device(0, "only", vram, vram, false)];
            let mut layers = [0u32; 2];
            for (slot, swa_full) in [false, true].into_iter().enumerate() {
                let plan = plan_smart_gpu_offload(
                    OffloadModel {
                        metadata,
                        costs: Some(&costs),
                        geometry: Some(&geometry),
                    },
                    OffloadRequest {
                        available_memory_bytes: machine.available_memory_bytes,
                        available_vram_bytes: Some(vram),
                        requested_context: Some(32768),
                        n_batch: 512,
                        resolved_offload_kqv: Some(true),
                        kv_types: KvCacheTypes::uniform(Some("f16")),
                        flash_attention_policy: FlashAttentionPolicy::Auto,
                        sidecar_vram_reserve_bytes: 0,
                        bundled_mtp_draft: false,
                        swa_full,
                    },
                    |_, _| None,
                );
                let info = run(
                    &PreviewRequest {
                        context_length: Some(32768),
                        swa_full: Some(swa_full),
                        ..request()
                    },
                    &machine,
                );
                if plan.estimated_gpu_layers > 0 {
                    assert_eq!(
                        gpu_pool(&info).expect("gpu pool").model_bytes,
                        costs.gpu_bytes(plan.estimated_gpu_layers),
                        "{vram} swa_full={swa_full}"
                    );
                }
                layers[slot] = plan.estimated_gpu_layers;
            }
            assert!(layers[1] <= layers[0], "{vram}");
            differing += u32::from(layers[1] != layers[0]);
        }
        assert!(differing > 0);
    }

    #[test]
    fn the_flash_policy_decides_the_attention_scratch_reserve() {
        let working = |flash| {
            let info = run(
                &PreviewRequest {
                    flash_attention: flash,
                    ..request()
                },
                &hardware(),
            );
            gpu_pool(&info).expect("gpu pool").working_bytes
        };
        assert_eq!(working(None), working(Some(FlashAttentionPolicy::Auto)));
        assert!(working(Some(FlashAttentionPolicy::Disabled)) > working(None));
    }

    #[test]
    fn a_measured_compute_buffer_replaces_the_working_reserve() {
        let machine = hardware();
        let sidecars = PreviewSidecars {
            vram_reserve_bytes: 100 * MIB,
            ..PreviewSidecars::default()
        };
        let partial = PreviewRequest {
            llama_gpu_layers: Some(4),
            ..request()
        };
        let probe = |_: u32, _: u32| {
            Some(MeasuredCompute {
                device_bytes: 777 * MIB,
                host_bytes: 5 * MIB,
            })
        };
        let measured = preview(&partial, &machine, sidecars, Some(&probe));
        assert_eq!(gpu_pool(&measured).expect("gpu").working_bytes, 877 * MIB);
        assert_eq!(measured.memory_fit[0].working_bytes, 5 * MIB);

        let formula = preview(&partial, &machine, sidecars, None);
        assert_ne!(gpu_pool(&formula).expect("gpu").working_bytes, 877 * MIB);
        let failed = |_: u32, _: u32| None;
        assert_eq!(
            preview(&partial, &machine, sidecars, Some(&failed)).memory_fit,
            formula.memory_fit
        );
    }

    #[test]
    fn the_plan_budgets_the_measured_compute_buffer() {
        let machine = hardware();
        let heavy = |_: u32, _: u32| {
            Some(MeasuredCompute {
                device_bytes: 4 * GIB,
                host_bytes: MIB,
            })
        };
        let estimated = run(&request(), &machine);
        let measured = preview(
            &request(),
            &machine,
            PreviewSidecars::default(),
            Some(&heavy),
        );
        assert!(placed_layers(&measured) < placed_layers(&estimated));
    }
}
