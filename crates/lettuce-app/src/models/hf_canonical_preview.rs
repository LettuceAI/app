use crate::{GgufHeaderSource, HuggingFaceBrowser, RemoteModel};
use lettuce_contracts as dto;
use lettuce_local_llm::context_preview::{
    LlamaCppContextInfo, PreviewDevice, PreviewHardware, PreviewRequest, PreviewSidecars,
    preview_context,
};
use lettuce_local_llm::offload::{
    KvCacheGeometry, KvCacheTypes, LlamaModelMetadata, ModelOffloadCosts, OffloadModel,
};
use lettuce_model_hub::{GgufPreviewMetadata, HfBrowseError};
use lettuce_settings::SecretStore;

pub(crate) const BROWSER_KV_TYPES: [&str; 14] = [
    "f32", "f16", "q8_1", "q8_0", "q6_k", "q5_k", "q5_1", "q5_0", "q4_k", "q4_1", "q4_0", "q3_k",
    "q2_k", "iq4_nl",
];

const UPGRADE_CEILING: f64 = 90.0;

#[derive(Clone, Debug)]
pub(crate) struct CanonicalModel {
    pub metadata: LlamaModelMetadata,
    pub costs: ModelOffloadCosts,
    pub geometry: KvCacheGeometry,
    pub quantized: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct CanonicalFileMetadata {
    pub model: Option<CanonicalModel>,
    pub nextn_predict_layers: Option<u32>,
}

fn immutable_revision(revision: &str) -> bool {
    [40, 64].contains(&revision.len()) && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(any(test, target_os = "android", target_os = "ios"))]
pub(crate) fn canonical_model(header: &GgufPreviewMetadata) -> Option<CanonicalModel> {
    use lettuce_local_llm::offload::KvLayerGeometry;
    if header.split_count != 1
        || header
            .scalar("attention.kv_lora_rank")
            .is_some_and(|value| value > 0)
    {
        return None;
    }
    let count = u32::try_from(header.scalar("block_count")?).ok()?;
    let layers_total = count as usize;
    let embedding = header.scalar("embedding_length")?;
    let context = u32::try_from(header.scalar("context_length")?).ok()?;
    let window = header
        .scalar("attention.sliding_window")
        .filter(|window| *window > 0);
    let swa_pattern = match window {
        Some(_) => {
            let pattern = header.values.get(&format!(
                "{}.attention.sliding_window_pattern",
                header.architecture
            ))?;
            if pattern.len() != layers_total {
                return None;
            }
            Some(pattern)
        }
        None => None,
    };
    let shared_kv_layers = header
        .scalar("attention.shared_kv_layers")
        .map_or(Some(0), |value| u32::try_from(value).ok())?;
    let own_cache_layers = layers_total.saturating_sub(shared_kv_layers as usize);
    if count == 0 || embedding == 0 || context == 0 || own_cache_layers == 0 {
        return None;
    }
    let mut layers = Vec::new();
    let mut first_heads = 0;
    for index in 0..layers_total {
        let heads = header.layer_value("attention.head_count", index, layers_total)?;
        if heads == 0 || embedding % heads != 0 {
            return None;
        }
        if index == 0 {
            first_heads = heads;
        }
        let n_head_kv = header
            .layer_value("attention.head_count_kv", index, layers_total)
            .unwrap_or(heads);
        let is_swa = swa_pattern.is_some_and(|pattern| pattern[index] != 0);
        let length = |suffix: &str, swa_suffix: &str| {
            is_swa
                .then(|| header.scalar(swa_suffix))
                .flatten()
                .or_else(|| header.scalar(suffix))
                .unwrap_or(embedding / heads)
        };
        let k = length("attention.key_length", "attention.key_length_swa");
        let v = length("attention.value_length", "attention.value_length_swa");
        if index < own_cache_layers {
            let key = header
                .tensors
                .iter()
                .find(|tensor| tensor.name == format!("blk.{index}.attn_k.weight"))?;
            let value = header
                .tensors
                .iter()
                .find(|tensor| tensor.name == format!("blk.{index}.attn_v.weight"))?;
            if key.dimensions.get(1).copied()? != n_head_kv.checked_mul(k)?
                || value.dimensions.get(1).copied()? != n_head_kv.checked_mul(v)?
            {
                return None;
            }
        }
        layers.push(KvLayerGeometry {
            n_head_kv: u32::try_from(n_head_kv).ok()?,
            n_embd_head_k: u32::try_from(k).ok()?,
            n_embd_head_v: u32::try_from(v).ok()?,
            is_swa,
        });
    }
    let first = *layers.first()?;
    let metadata = LlamaModelMetadata {
        model_size_bytes: header.model_bytes()?,
        layer_count: count,
        nextn_layer_count: u32::try_from(header.scalar("nextn_predict_layers").unwrap_or(0))
            .ok()?,
        max_context_length: context,
        n_embd: embedding,
        n_head: first_heads,
        n_head_kv: u64::from(first.n_head_kv),
        n_embd_head_k: header
            .scalar("attention.key_length")
            .unwrap_or(embedding / first_heads),
        n_embd_head_v: header
            .scalar("attention.value_length")
            .unwrap_or(embedding / first_heads),
    };
    let costs = ModelOffloadCosts::from_tensors(
        header
            .tensors
            .iter()
            .map(|tensor| (tensor.name.as_str(), tensor.bytes)),
    )?;
    if costs.unit_count() != metadata.offload_layer_count() {
        return None;
    }
    let quantized = header.tensors.iter().any(|tensor| {
        tensor.dimensions.len() > 1
            && lettuce_model_hub::gguf_tensor_storage(tensor.kind)
                .is_some_and(|(block, _)| block > 1)
    });
    Some(CanonicalModel {
        metadata,
        costs,
        geometry: KvCacheGeometry::new(layers, u32::try_from(window.unwrap_or(0)).ok()?)?
            .with_shared_kv_layers(shared_kv_layers),
        quantized,
    })
}

impl HuggingFaceBrowser {
    pub(crate) async fn canonical_file<S: SecretStore + ?Sized, H: GgufHeaderSource + ?Sized>(
        &self,
        secrets: &S,
        source: &H,
        model: RemoteModel<'_>,
        file: &dto::HfRunnabilityFile,
    ) -> Result<Option<std::sync::Arc<CanonicalFileMetadata>>, HfBrowseError> {
        let key = (
            model.id.to_owned(),
            model.revision.to_owned(),
            file.filename.clone(),
        );
        let pinned = immutable_revision(model.revision);
        if pinned
            && let Some(cached) = self
                .preview_models
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&key)
                .cloned()
        {
            return Ok(Some(cached));
        }
        let parts = if file.parts.is_empty() {
            vec![dto::HfFilePart {
                filename: file.filename.clone(),
                size: file.size,
            }]
        } else {
            file.parts.clone()
        };
        let shard = lettuce_model_hub::GgufShardName::parse(&file.filename);
        if let Some(shard) = &shard {
            if shard.index != 1
                || parts.len() != shard.count as usize
                || parts.iter().enumerate().any(|(index, part)| {
                    part.filename != shard.filename(index as u32 + 1) || part.size == 0
                })
            {
                return Ok(None);
            }
        } else if parts.len() != 1 || parts[0].filename != file.filename {
            return Ok(None);
        }
        let mut headers = Vec::new();
        for part in &parts {
            let physical = dto::HfRunnabilityFile {
                filename: part.filename.clone(),
                size: part.size,
                parts: Vec::new(),
            };
            let Some((bytes, header)) = self
                .complete_preview_header(secrets, source, model, &physical)
                .await?
            else {
                return Ok(None);
            };
            if header.split_count != parts.len() as u64 || header.split_no != headers.len() as u64 {
                return Ok(None);
            }
            headers.push((part.filename.clone(), bytes, header));
        }
        let mut header = headers[0].2.clone();
        if header.architecture.is_empty() {
            return Ok(None);
        }
        let mut names = std::collections::HashSet::new();
        for (_, _, part) in &headers {
            if (!part.architecture.is_empty() && part.architecture != header.architecture)
                || part
                    .values
                    .iter()
                    .any(|(key, value)| header.values.get(key) != Some(value))
                || part.split_tensors_count != header.split_tensors_count
                || part
                    .tensors
                    .iter()
                    .any(|tensor| !names.insert(tensor.name.clone()))
            {
                return Ok(None);
            }
        }
        header.tensors = headers
            .iter()
            .flat_map(|(_, _, part)| part.tensors.clone())
            .collect();
        if parts.len() > 1 && header.split_tensors_count != Some(header.tensors.len() as u64) {
            return Ok(None);
        }
        header.split_count = 1;
        let native = complete_model_parts(&headers, &header);
        let nextn_predict_layers = native
            .as_ref()
            .map(|native| native.metadata.nextn_layer_count)
            .or_else(|| {
                header
                    .scalar("nextn_predict_layers")
                    .and_then(|value| u32::try_from(value).ok())
            });
        let metadata = std::sync::Arc::new(CanonicalFileMetadata {
            model: native,
            nextn_predict_layers,
        });
        if pinned && metadata.model.is_some() {
            self.preview_models
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(key, metadata.clone());
        }
        Ok(Some(metadata))
    }

    pub(crate) async fn complete_preview_header<
        S: SecretStore + ?Sized,
        H: GgufHeaderSource + ?Sized,
    >(
        &self,
        secrets: &S,
        source: &H,
        model: RemoteModel<'_>,
        file: &dto::HfRunnabilityFile,
    ) -> Result<Option<(Vec<u8>, GgufPreviewMetadata)>, HfBrowseError> {
        let key = (
            model.id.to_owned(),
            model.revision.to_owned(),
            file.filename.clone(),
        );
        let pinned = immutable_revision(model.revision);
        if pinned
            && let Some(bytes) = self
                .preview_headers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&key)
                .cloned()
            && let Some(metadata) = lettuce_model_hub::parse_gguf_preview(&bytes)
        {
            return Ok(Some((bytes, metadata)));
        }
        let token = Self::saved_token(secrets).await?;
        let reading = async {
            let budget =
                lettuce_model_hub::GGUF_PREVIEW_METADATA_BYTES.min(file.size.saturating_sub(1));
            if budget < 24 {
                return None;
            }
            let mut length = lettuce_model_hub::GGUF_HEADER_PROBE_BYTES.min(budget);
            let mut bytes = Vec::new();
            loop {
                let offset = bytes.len() as u64;
                let additional = source
                    .read_range(
                        model.id,
                        model.revision,
                        &file.filename,
                        (offset, length.checked_sub(offset)?),
                        file.size,
                        token.as_ref(),
                    )
                    .await?;
                if additional.len() as u64 > length.checked_sub(offset)? {
                    return None;
                }
                bytes.extend(additional);
                if let Some(header) = lettuce_model_hub::parse_gguf_preview(&bytes) {
                    if header.tensor_data_offset <= lettuce_model_hub::GGUF_PREVIEW_METADATA_BYTES
                        && header.tensors.iter().all(|tensor| {
                            header
                                .tensor_data_offset
                                .checked_add(tensor.offset)
                                .and_then(|offset| offset.checked_add(tensor.bytes))
                                .is_some_and(|end| end <= file.size)
                        })
                    {
                        return Some((bytes, header));
                    }
                    return None;
                }
                if length >= budget || bytes.len() < length as usize {
                    return None;
                }
                match lettuce_model_hub::gguf_preview_boundary(&bytes) {
                    lettuce_model_hub::GgufHeaderBoundary::Incomplete {
                        required_prefix_bytes,
                    } => {
                        let required = u64::try_from(required_prefix_bytes).ok()?;
                        if required > budget {
                            return None;
                        }
                        length = length.saturating_mul(2).max(required).min(budget);
                    }
                    lettuce_model_hub::GgufHeaderBoundary::Complete { .. }
                    | lettuce_model_hub::GgufHeaderBoundary::Invalid => return None,
                }
            }
        };
        let parsed = tokio::time::timeout(lettuce_network::BROWSE_TIMEOUT, reading)
            .await
            .ok()
            .flatten();
        if pinned && let Some((bytes, _)) = &parsed {
            self.preview_headers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(key, bytes.clone());
        }
        Ok(parsed)
    }
}

pub(crate) fn preview_label(info: &LlamaCppContextInfo) -> Option<dto::RunnabilityLabel> {
    if info.memory_fit.is_empty() || info.available_memory_bytes.is_none() {
        return None;
    }
    if !info.placement_fits {
        return Some(dto::RunnabilityLabel::Unrunnable);
    }
    if info.memory_fit.iter().any(|pool| {
        pool.model_bytes
            .saturating_add(pool.context_bytes)
            .saturating_add(pool.working_bytes)
            > pool.available_bytes
    }) {
        return Some(dto::RunnabilityLabel::Unrunnable);
    }
    let gpu_layers = info.estimated_placement.as_ref()?.total_gpu_layers;
    Some(if gpu_layers == info.max_gpu_layers? {
        dto::RunnabilityLabel::Excellent
    } else if gpu_layers > 0 {
        dto::RunnabilityLabel::Good
    } else {
        dto::RunnabilityLabel::Poor
    })
}

pub(crate) fn preview_policy(settings: &lettuce_models::LlamaCppSettings) -> dto::HfGpuPolicy {
    dto::HfGpuPolicy {
        multi_gpu_enabled: settings.multi_gpu_enabled.unwrap_or(false),
        gpu_device_ids: settings.gpu_device_ids.clone().unwrap_or_default(),
        single_gpu_device_id: settings.single_gpu_device_id,
        distribution: settings
            .gpu_distribution_mode
            .as_ref()
            .and_then(|mode| serde_json::to_value(mode).ok())
            .and_then(|value| value.as_str().map(str::to_owned)),
        manual_layers: settings
            .gpu_manual_layers
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|layer| dto::LlamaGpuLayers {
                device_id: layer.device_id,
                layers: layer.layers,
            })
            .collect(),
        priority_vram_limit_bytes: settings.priority_vram_limit_bytes,
    }
}

pub(crate) fn preview_options(
    policy: &dto::HfGpuPolicy,
    choice: Option<&dto::HfPlanChoice>,
    settings: &lettuce_models::LlamaCppSettings,
) -> PreviewRequest {
    PreviewRequest {
        context_length: choice
            .and_then(|choice| u32::try_from(choice.context_length).ok())
            .or(Some(8192)),
        batch_size: Some(
            settings
                .ubatch_size
                .unwrap_or(settings.batch_size.unwrap_or(512))
                .min(settings.batch_size.unwrap_or(512)),
        ),
        flash_attention: settings.flash_attention.map(|policy| match policy {
            lettuce_models::LlamaFlashAttention::Auto => {
                lettuce_local_llm::offload::FlashAttentionPolicy::Auto
            }
            lettuce_models::LlamaFlashAttention::Enabled => {
                lettuce_local_llm::offload::FlashAttentionPolicy::Enabled
            }
            lettuce_models::LlamaFlashAttention::Disabled => {
                lettuce_local_llm::offload::FlashAttentionPolicy::Disabled
            }
        }),
        swa_full: settings.swa_full,
        llama_offload_kqv: Some(
            choice.is_none_or(|choice| choice.kv_placement != dto::HfKvPlacement::Ram),
        ),
        llama_kv_type: Some(
            choice
                .map_or("q8_0", |choice| choice.kv_type.trim())
                .to_ascii_lowercase(),
        ),
        llama_gpu_layers: match choice.map(|choice| choice.model_offload) {
            Some(dto::HfModelOffload::Cpu) => Some(0),
            Some(dto::HfModelOffload::Gpu) => Some(u32::MAX),
            Some(dto::HfModelOffload::Mixed) => None,
            _ => settings.gpu_layers,
        },
        llama_multi_gpu_enabled: Some(policy.multi_gpu_enabled),
        llama_gpu_device_ids: Some(
            policy
                .gpu_device_ids
                .iter()
                .map(|id| *id as usize)
                .collect(),
        ),
        llama_gpu_distribution_mode: policy.distribution.clone(),
        llama_gpu_manual_layers: Some(
            policy
                .manual_layers
                .iter()
                .map(
                    |layer| lettuce_local_llm::context_preview::GpuLayerAssignment {
                        device_id: layer.device_id as usize,
                        layers: layer.layers,
                    },
                )
                .collect(),
        ),
        llama_single_gpu_device_id: policy.single_gpu_device_id.map(|id| id as usize),
        llama_priority_vram_limit_bytes: policy.priority_vram_limit_bytes,
        ..PreviewRequest::default()
    }
}

pub(crate) fn local_preview_hardware() -> Option<PreviewHardware> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Some(PreviewHardware {
            available_memory_bytes: lettuce_local_llm::hardware::get_available_memory_bytes(),
            total_memory_bytes: lettuce_local_llm::hardware::get_total_memory_bytes(),
            available_vram_bytes: lettuce_local_llm::hardware::get_available_vram_bytes(),
            supports_gpu_offload: lettuce_local_llm::llama::shared_backend()
                .ok()?
                .supports_gpu_offload(),
            unified_memory: lettuce_local_llm::hardware::is_unified_memory(),
            devices: lettuce_local_llm::hardware::list_gpu_devices()
                .into_iter()
                .map(|device| PreviewDevice {
                    index: device.index,
                    name: device.name,
                    memory_free: device.memory_free,
                    memory_total: device.memory_total,
                    integrated: device.device_type == "IntegratedGpu",
                })
                .collect(),
        })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        None
    }
}

pub(crate) fn sprout_preview_hardware(bytes: &[u8]) -> Result<PreviewHardware, String> {
    let specs = lettuce_model_hub::sprout_preview_specs("Sprout", bytes)?;
    let devices: Vec<_> = specs
        .gpus
        .into_iter()
        .enumerate()
        .filter(|(_, gpu)| {
            ["Gpu", "IntegratedGpu", "GPU", "Accelerator"].contains(&gpu.device_type.as_str())
        })
        .map(|(index, gpu)| PreviewDevice {
            index: gpu.index.map_or(index, |id| id as usize),
            name: gpu.name,
            memory_free: gpu.memory_free,
            memory_total: gpu.memory_total,
            integrated: gpu.device_type == "IntegratedGpu",
        })
        .collect();
    let unified = specs.unified_memory
        || !devices.is_empty() && devices.iter().all(|device| device.integrated);
    Ok(PreviewHardware {
        available_memory_bytes: (specs.available_memory_bytes > 0)
            .then_some(specs.available_memory_bytes),
        total_memory_bytes: specs.total_memory_bytes,
        available_vram_bytes: devices.iter().map(|device| device.memory_free).max(),
        supports_gpu_offload: !devices.is_empty(),
        unified_memory: unified,
        devices,
    })
}

pub(crate) fn plan_preview(
    model: &CanonicalModel,
    options: &PreviewRequest,
    hardware: &PreviewHardware,
    sidecars: PreviewSidecars,
) -> LlamaCppContextInfo {
    preview_context(
        OffloadModel {
            metadata: model.metadata,
            costs: Some(&model.costs),
            geometry: Some(&model.geometry),
        },
        options,
        hardware,
        sidecars,
        None,
    )
}

/// The placement a plan amounts to. Every offload unit on the GPU counts as
/// the whole model there, since llama.cpp keeps the input embedding in host
/// memory whatever the offload.
pub(crate) fn classify_gpu_mode(
    memory_fit: &[dto::HfCanonicalMemoryFit],
    gpu_layers: u32,
    total_layers: u32,
) -> dto::HfPlanGpuMode {
    let sum = |gpu: bool, bytes: fn(&dto::HfCanonicalMemoryFit) -> u64| {
        memory_fit
            .iter()
            .filter(|pool| pool.gpu == gpu)
            .map(bytes)
            .fold(0u64, u64::saturating_add)
    };
    let gpu_model = sum(true, |pool| pool.model_bytes);
    let model = gpu_model.saturating_add(sum(false, |pool| pool.model_bytes));
    let on_gpu = if model > 0 {
        gpu_model as f64 / model as f64
    } else {
        0.0
    };
    if gpu_layers == 0 || on_gpu == 0.0 {
        return dto::HfPlanGpuMode::Cpu;
    }
    let all_layers = total_layers > 0 && gpu_layers >= total_layers;
    if !all_layers && on_gpu < 0.999 {
        return if on_gpu >= 0.75 {
            dto::HfPlanGpuMode::MostLayers
        } else if on_gpu >= 0.5 {
            dto::HfPlanGpuMode::HalfLayers
        } else {
            dto::HfPlanGpuMode::FewLayers
        };
    }
    let gpu_context = sum(true, |pool| pool.context_bytes);
    let context = gpu_context.saturating_add(sum(false, |pool| pool.context_bytes));
    if context == 0 || gpu_context >= context {
        dto::HfPlanGpuMode::Full
    } else if gpu_context as f64 / context as f64 >= 0.8 {
        dto::HfPlanGpuMode::NearFull
    } else {
        dto::HfPlanGpuMode::KvSpill
    }
}

fn planner_mode(mode: dto::HfPlanGpuMode) -> lettuce_model_hub::PlannerGpuMode {
    use lettuce_model_hub::PlannerGpuMode;
    match mode {
        dto::HfPlanGpuMode::Full => PlannerGpuMode::Full,
        dto::HfPlanGpuMode::NearFull => PlannerGpuMode::NearFull,
        dto::HfPlanGpuMode::KvSpill => PlannerGpuMode::KvSpill,
        dto::HfPlanGpuMode::KvHeavySpill => PlannerGpuMode::KvHeavySpill,
        dto::HfPlanGpuMode::RamModelVramCtx => PlannerGpuMode::RamModelVramCtx,
        dto::HfPlanGpuMode::RamModelRamCtx => PlannerGpuMode::RamModelRamCtx,
        dto::HfPlanGpuMode::MostLayers => PlannerGpuMode::MostLayers,
        dto::HfPlanGpuMode::HalfLayers => PlannerGpuMode::HalfLayers,
        dto::HfPlanGpuMode::FewLayers => PlannerGpuMode::FewLayers,
        dto::HfPlanGpuMode::Cpu => PlannerGpuMode::Cpu,
        dto::HfPlanGpuMode::GpuUnavailable => PlannerGpuMode::GpuUnavailable,
    }
}

pub(crate) fn placement_speeds(
    mode: dto::HfPlanGpuMode,
    unified_memory: bool,
) -> (Option<dto::HfSpeed>, Option<dto::HfSpeed>) {
    if unified_memory {
        return (None, None);
    }
    let speed = |speed| match speed {
        lettuce_model_hub::PlannerSpeed::Fast => dto::HfSpeed::Fast,
        lettuce_model_hub::PlannerSpeed::Medium => dto::HfSpeed::Medium,
        lettuce_model_hub::PlannerSpeed::Slow => dto::HfSpeed::Slow,
    };
    let (prefill, generation) = lettuce_model_hub::planner_performance(planner_mode(mode));
    (Some(speed(prefill)), Some(speed(generation)))
}

pub(crate) fn canonical_plan(
    filename: &str,
    context: u32,
    policy: &dto::HfGpuPolicy,
    hardware: &PreviewHardware,
    info: LlamaCppContextInfo,
) -> Option<dto::HfCanonicalPlan> {
    let label = preview_label(&info)?;
    let placement = info.estimated_placement.as_ref()?;
    let gpu_layers = placement.total_gpu_layers;
    let offload_kqv = gpu_layers > 0
        && info
            .memory_fit
            .iter()
            .any(|pool| (pool.gpu || hardware.unified_memory) && pool.context_bytes > 0);
    let total_layers = info.max_gpu_layers?;
    let placement = dto::LlamaPlacement {
        total_gpu_layers: gpu_layers,
        per_device_layers: placement.per_device_layers.clone(),
    };
    let memory_fit: Vec<_> = info
        .memory_fit
        .into_iter()
        .map(|pool| dto::HfCanonicalMemoryFit {
            gpu: pool.gpu,
            devices: pool.devices,
            capacity_bytes: (pool.capacity_bytes > 0).then_some(pool.capacity_bytes),
            available_bytes: pool.available_bytes,
            model_bytes: pool.model_bytes,
            context_bytes: pool.context_bytes,
            working_bytes: pool.working_bytes,
        })
        .collect();
    let gpu_mode = classify_gpu_mode(&memory_fit, gpu_layers, total_layers);
    let (prefill_speed, generation_speed) = placement_speeds(gpu_mode, hardware.unified_memory);
    Some(dto::HfCanonicalPlan {
        unified_memory: hardware.unified_memory,
        filename: filename.to_owned(),
        context_length: context,
        label,
        gpu_layers,
        total_layers,
        offload_kqv,
        gpu_mode,
        prefill_speed,
        generation_speed,
        memory_fit,
        gpu_policy: policy.clone(),
        placement: Some(placement),
    })
}

#[cfg(test)]
fn complete_model(bytes: &[u8], header: &GgufPreviewMetadata) -> Option<CanonicalModel> {
    complete_model_parts(
        &[("model.gguf".to_owned(), bytes.to_vec(), header.clone())],
        header,
    )
}

fn complete_model_parts(
    parts: &[(String, Vec<u8>, GgufPreviewMetadata)],
    combined: &GgufPreviewMetadata,
) -> Option<CanonicalModel> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if combined.tensors.iter().any(|tensor| {
            lettuce_local_llm::llama::gguf_tensor_storage(tensor.kind)
                != lettuce_model_hub::gguf_tensor_storage(tensor.kind)
        }) {
            return None;
        }
        let root =
            std::env::temp_dir().join(format!("lettuce-hf-preview-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).ok()?;
        let loaded = (|| {
            for (filename, bytes, header) in parts {
                if header.tensor_data_offset > lettuce_model_hub::GGUF_PREVIEW_METADATA_BYTES
                    || lettuce_model_hub::resolve_url("test/model", "main", filename).is_err()
                {
                    return None;
                }
                let path = root.join(filename);
                std::fs::create_dir_all(path.parent()?).ok()?;
                let mut file = std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(path)
                    .ok()?;
                let mut prefix = bytes
                    .get(
                        ..usize::try_from(header.tensor_data_offset)
                            .ok()?
                            .min(bytes.len()),
                    )?
                    .to_vec();
                prefix.resize(usize::try_from(header.tensor_data_offset).ok()?, 0);
                let extent =
                    header
                        .tensors
                        .iter()
                        .try_fold(header.tensor_data_offset, |end, tensor| {
                            header
                                .tensor_data_offset
                                .checked_add(tensor.offset)?
                                .checked_add(tensor.bytes)
                                .map(|next| end.max(next))
                        })?;
                use std::io::Write;
                file.write_all(&prefix).ok()?;
                file.set_len(extent).ok()?;
            }
            let loaded = lettuce_local_llm::llama::load_model_preview(
                root.join(&parts.first()?.0).to_str()?,
            )
            .ok()?;
            Some(CanonicalModel {
                metadata: loaded.metadata,
                costs: ModelOffloadCosts::from_tensors(
                    combined
                        .tensors
                        .iter()
                        .map(|tensor| (tensor.name.as_str(), tensor.bytes)),
                )?,
                geometry: loaded.geometry?,
                quantized: combined.tensors.iter().any(|tensor| {
                    tensor.dimensions.len() > 1
                        && lettuce_model_hub::gguf_tensor_storage(tensor.kind)
                            .is_some_and(|(block, _)| block > 1)
                }),
            })
        })();
        let _ = std::fs::remove_dir_all(root);
        loaded
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = parts;
        canonical_model(combined)
    }
}

fn automatic_probe_budget(options: &PreviewRequest, hardware: &PreviewHardware) -> Option<u64> {
    let ram = hardware.available_memory_bytes?;
    if hardware.unified_memory
        || !hardware.supports_gpu_offload
        || options.llama_gpu_layers == Some(0)
    {
        return Some(ram);
    }
    let ids = if options.llama_multi_gpu_enabled == Some(true)
        && options.llama_single_gpu_device_id.is_none()
    {
        let ids = options.llama_gpu_device_ids.as_ref()?;
        if ids.len() < 2 || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len()
        {
            return None;
        }
        ids.clone()
    } else {
        vec![options.llama_single_gpu_device_id?]
    };
    ids.into_iter().try_fold(ram, |available, id| {
        let device = hardware.devices.iter().find(|device| device.index == id)?;
        if device.integrated {
            return Some(available);
        }
        available.checked_add(device.memory_free)
    })
}

struct UpgradeSearch<'a> {
    options: &'a PreviewRequest,
    policy: &'a dto::HfGpuPolicy,
    hardware: &'a PreviewHardware,
    sidecars: PreviewSidecars,
    probe_budget: Option<u64>,
}

async fn upgrade_file<S: SecretStore + ?Sized, H: GgufHeaderSource + ?Sized>(
    browser: &HuggingFaceBrowser,
    secrets: &S,
    source: &H,
    model: RemoteModel<'_>,
    files: &[dto::HfRunnabilityFile],
    selected: &dto::HfCanonicalPlan,
    search: UpgradeSearch<'_>,
) -> Result<Option<String>, HfBrowseError> {
    let UpgradeSearch {
        options,
        policy,
        hardware,
        sidecars,
        probe_budget,
    } = search;
    let quality = |filename: &str| {
        lettuce_model_hub::quant_quality_score(&lettuce_model_hub::extract_quantization(filename))
    };
    let current = quality(&selected.filename);
    if current >= UPGRADE_CEILING {
        return Ok(None);
    }
    let mut better: Vec<_> = files
        .iter()
        .filter(|file| {
            file.filename != selected.filename
                && quality(&file.filename) > current
                && file.size > 0
                && probe_budget.is_some_and(|budget| file.size <= budget)
        })
        .collect();
    better.sort_by(|a, b| {
        quality(&b.filename)
            .total_cmp(&quality(&a.filename))
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.filename.cmp(&b.filename))
    });
    for file in better {
        let Some(parsed) = browser.canonical_file(secrets, source, model, file).await? else {
            continue;
        };
        let Some(native) = parsed.model.as_ref() else {
            continue;
        };
        let context = options
            .context_length
            .unwrap_or(8192)
            .min(native.metadata.max_context_length);
        let info = plan_preview(native, options, hardware, sidecars);
        let Some(plan) = canonical_plan(&file.filename, context, policy, hardware, info) else {
            continue;
        };
        if matches!(
            plan.label,
            dto::RunnabilityLabel::Excellent | dto::RunnabilityLabel::Good
        ) && plan.gpu_mode == selected.gpu_mode
        {
            return Ok(Some(file.filename.clone()));
        }
    }
    Ok(None)
}

pub(crate) async fn canonical_recommendation<
    S: SecretStore + ?Sized,
    H: GgufHeaderSource + ?Sized,
>(
    browser: &HuggingFaceBrowser,
    secrets: &S,
    source: &H,
    model: RemoteModel<'_>,
    request: &dto::HfRecommendationRequest,
    hardware: &PreviewHardware,
    defaults: &lettuce_models::LlamaCppSettings,
) -> Result<dto::HfCanonicalRecommendation, HfBrowseError> {
    let mut policy = request
        .gpu_policy
        .clone()
        .unwrap_or_else(|| preview_policy(defaults));
    if !policy.multi_gpu_enabled && policy.single_gpu_device_id.is_none() {
        policy.single_gpu_device_id = hardware
            .devices
            .iter()
            .max_by_key(|device| device.memory_free)
            .map(|device| device.index as u32);
    }
    let mut options = preview_options(&policy, request.plan.as_ref(), defaults);
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if request.ollama_account_id.is_none() && options.flash_attention.is_none() {
        options.flash_attention = Some(lettuce_local_llm::context_info::flash_attention_policy());
    }
    if !hardware.supports_gpu_offload {
        options.llama_gpu_layers = Some(0);
        options.llama_offload_kqv = Some(false);
    }
    let mmproj_bytes = request
        .sidecars
        .as_ref()
        .and_then(|sidecars| sidecars.mmproj.as_ref())
        .map_or(0, |file| file.size);
    let mut sidecars = PreviewSidecars {
        vram_reserve_bytes: mmproj_bytes,
        bundled_mtp_draft: request
            .sidecars
            .as_ref()
            .is_some_and(|sidecars| sidecars.mtp_bundled),
        ..PreviewSidecars::default()
    };
    let mut sidecars_known = true;
    if let Some(draft) = request
        .sidecars
        .as_ref()
        .and_then(|sidecars| sidecars.mtp.as_ref())
    {
        if let Some(parsed) = browser
            .canonical_file(secrets, source, model, draft)
            .await?
        {
            if let Some(draft) = parsed.model.as_ref() {
                let reserve = lettuce_local_llm::offload::estimate_mtp_gpu_reserve_bytes(
                    &draft.metadata,
                    Some(&draft.geometry),
                    16384,
                    512,
                    KvCacheTypes::from_settings(Some("q8_0"), None, None),
                );
                sidecars.mtp_vram_reserve_bytes = reserve;
                sidecars.vram_reserve_bytes = sidecars.vram_reserve_bytes.saturating_add(reserve);
            } else {
                sidecars_known = false;
            }
        } else {
            sidecars_known = false;
        }
    }
    let mut reports = Vec::new();
    let mut eligible = Vec::new();
    let mut selected = None;
    let named_float = |file: &dto::HfRunnabilityFile| {
        matches!(
            lettuce_model_hub::extract_quantization(&file.filename)
                .to_uppercase()
                .as_str(),
            "F16" | "BF16" | "F32" | "F64" | "UD-BF16"
        )
    };
    let quantized_present = request.files.iter().any(|file| !named_float(file));
    let probe_budget = automatic_probe_budget(&options, hardware);
    let mut candidates: Vec<_> = request
        .files
        .iter()
        .filter(|file| {
            request.plan.as_ref().map_or_else(
                || {
                    (!quantized_present || !named_float(file))
                        && file.size > 0
                        && probe_budget.is_some_and(|budget| file.size <= budget)
                },
                |choice| choice.filename == file.filename,
            )
        })
        .collect();
    candidates.sort_by(|a, b| {
        lettuce_model_hub::quant_quality_score(&lettuce_model_hub::extract_quantization(
            &b.filename,
        ))
        .total_cmp(&lettuce_model_hub::quant_quality_score(
            &lettuce_model_hub::extract_quantization(&a.filename),
        ))
        .then_with(|| b.size.cmp(&a.size))
        .then_with(|| a.filename.cmp(&b.filename))
    });
    for file in candidates {
        let parsed = browser.canonical_file(secrets, source, model, file).await?;
        let native = parsed.as_ref().and_then(|parsed| parsed.model.as_ref());
        let quantization = lettuce_model_hub::extract_quantization(&file.filename);
        let Some(native) = native.filter(|_| sidecars_known) else {
            reports.push(dto::HfCanonicalFile {
                nextn_predict_layers: parsed
                    .as_ref()
                    .and_then(|parsed| parsed.nextn_predict_layers),
                filename: file.filename.clone(),
                label: None,
                max_context: None,
                recommended_context: None,
                gpu_optimal_context: None,
                kv_types: Vec::new(),
            });
            if request.plan.is_none() {
                break;
            }
            continue;
        };
        let info = plan_preview(native, &options, hardware, sidecars);
        let label = preview_label(&info);
        let initial_kv = (native.metadata.max_context_length >= 8192)
            .then(|| {
                ["q8_0", "q6_k"].into_iter().find(|kv| {
                    let mut initial_options = options.clone();
                    initial_options.context_length = Some(8192);
                    initial_options.llama_kv_type = Some((*kv).to_owned());
                    preview_label(&plan_preview(native, &initial_options, hardware, sidecars))
                        .is_some_and(|label| label != dto::RunnabilityLabel::Unrunnable)
                })
            })
            .flatten();
        let recommendation_fits = if request.plan.is_some() {
            label.is_some_and(|label| label != dto::RunnabilityLabel::Unrunnable)
        } else {
            initial_kv.is_some()
        };
        if recommendation_fits && (!quantized_present || native.quantized) {
            eligible.push((
                file.filename.clone(),
                native.quantized,
                lettuce_model_hub::quant_quality_score(&quantization),
                file.size,
                request
                    .plan
                    .as_ref()
                    .map(|choice| choice.kv_type.clone())
                    .or_else(|| initial_kv.map(str::to_owned)),
            ));
        }
        let mut optimal_options = options.clone();
        optimal_options.context_length = None;
        let optimal = plan_preview(native, &optimal_options, hardware, sidecars);
        let context = options
            .context_length
            .unwrap_or(8192)
            .min(native.metadata.max_context_length);
        let mut gpu_options = optimal_options.clone();
        gpu_options.llama_gpu_layers = Some(u32::MAX);
        gpu_options.llama_offload_kqv = Some(true);
        let gpu = plan_preview(native, &gpu_options, hardware, sidecars);
        reports.push(dto::HfCanonicalFile {
            nextn_predict_layers: Some(native.metadata.nextn_layer_count),
            filename: file.filename.clone(),
            label,
            max_context: Some(native.metadata.max_context_length),
            recommended_context: optimal.recommended_context_length,
            gpu_optimal_context: gpu
                .recommended_context_length
                .filter(|_| hardware.supports_gpu_offload),
            kv_types: BROWSER_KV_TYPES
                .iter()
                .map(|kv_type| {
                    let mut kv_options = options.clone();
                    kv_options.llama_kv_type = Some((*kv_type).to_owned());
                    let current = plan_preview(native, &kv_options, hardware, sidecars);
                    kv_options.context_length = None;
                    let limit = plan_preview(native, &kv_options, hardware, sidecars);
                    dto::HfCanonicalKv {
                        kv_type: (*kv_type).to_owned(),
                        cache_bytes: current
                            .memory_fit
                            .iter()
                            .map(|pool| pool.context_bytes)
                            .fold(0u64, u64::saturating_add),
                        recommended_context: limit.recommended_context_length,
                    }
                })
                .collect(),
        });
        if request
            .plan
            .as_ref()
            .is_some_and(|choice| choice.filename == file.filename)
        {
            selected = canonical_plan(&file.filename, context, &policy, hardware, info);
        }
        if request.plan.is_none() && !eligible.is_empty() {
            break;
        }
    }

    let upgrade = match selected.as_ref() {
        Some(selected) => {
            upgrade_file(
                browser,
                secrets,
                source,
                model,
                &request.files,
                selected,
                UpgradeSearch {
                    options: &options,
                    policy: &policy,
                    hardware,
                    sidecars,
                    probe_budget,
                },
            )
            .await?
        }
        None => None,
    };
    eligible.retain(|(_, quantized, _, _, _)| !quantized_present || *quantized);
    eligible.sort_by(|a, b| {
        b.2.total_cmp(&a.2)
            .then_with(|| b.3.cmp(&a.3))
            .then_with(|| a.0.cmp(&b.0))
    });
    Ok(dto::HfCanonicalRecommendation {
        devices: hardware
            .devices
            .iter()
            .map(|device| dto::LlamaDevice {
                index: device.index as u32,
                name: device.name.clone(),
                description: String::new(),
                backend: String::new(),
                memory_total: device.memory_total,
                memory_free: device.memory_free,
                device_type: if device.integrated {
                    "IntegratedGpu"
                } else {
                    "Gpu"
                }
                .to_owned(),
            })
            .collect(),
        files: reports,
        selected,
        recommended_filename: eligible.first().map(|item| item.0.clone()),
        recommended_kv_type: eligible.first().and_then(|item| item.4.clone()),
        upgrade,
        gpu_policy: policy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use lettuce_settings::{InMemorySecretStore, SecretValue};
    use std::sync::Mutex;
    const FIXTURE: &[u8] =
        include_bytes!("../../../lettuce-model-hub/tests/fixtures/preview-llama.gguf");

    #[test]
    fn mixed_placement_uses_the_current_fit_instead_of_a_global_cpu_override() {
        let defaults = lettuce_models::LlamaCppSettings {
            gpu_layers: Some(0),
            ..Default::default()
        };
        let choice = dto::HfPlanChoice {
            filename: "model.gguf".to_owned(),
            context_length: 8192,
            kv_type: "q8_0".to_owned(),
            model_offload: dto::HfModelOffload::Mixed,
            kv_placement: dto::HfKvPlacement::Auto,
        };
        let options = preview_options(&preview_policy(&defaults), Some(&choice), &defaults);
        let model =
            canonical_model(&lettuce_model_hub::parse_gguf_preview(FIXTURE).expect("header"))
                .expect("model");
        let info = plan_preview(&model, &options, &hardware(), PreviewSidecars::default());
        assert!(
            info.estimated_placement
                .expect("placement")
                .total_gpu_layers
                > 0
        );
    }

    fn hardware() -> PreviewHardware {
        PreviewHardware {
            available_memory_bytes: Some(16 * 1024 * 1024 * 1024),
            total_memory_bytes: 32 * 1024 * 1024 * 1024,
            available_vram_bytes: Some(4 * 1024 * 1024 * 1024),
            supports_gpu_offload: true,
            unified_memory: false,
            devices: vec![
                PreviewDevice {
                    index: 2,
                    name: "first".into(),
                    memory_free: 4 * 1024 * 1024 * 1024,
                    memory_total: 8 * 1024 * 1024 * 1024,
                    integrated: false,
                },
                PreviewDevice {
                    index: 7,
                    name: "second".into(),
                    memory_free: 2 * 1024 * 1024 * 1024,
                    memory_total: 4 * 1024 * 1024 * 1024,
                    integrated: false,
                },
            ],
        }
    }

    #[test]
    fn actual_gguf_tensor_directory_and_native_metadata_loader_feed_the_same_planner() {
        let header = lettuce_model_hub::parse_gguf_preview(FIXTURE).expect("valid GGUF");
        let pure = canonical_model(&header).expect("structurally validated geometry");
        let path = std::env::temp_dir().join(format!(
            "lettuce-preview-parity-{}.gguf",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path, &FIXTURE[..header.tensor_data_offset as usize])
            .expect("header-only fixture");
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("sparse fixture")
            .set_len(FIXTURE.len() as u64)
            .expect("logical tensor bounds without weight writes");
        let loaded = lettuce_local_llm::llama::load_model_preview(path.to_str().expect("path"))
            .expect("native metadata-only loader accepts complete index without model weights");
        std::fs::remove_file(path).expect("remove fixture");
        assert_eq!(loaded.metadata, pure.metadata);
        let costs = loaded.costs.expect("native tensor costs");
        for layers in 0..=pure.metadata.offload_layer_count() {
            assert_eq!(costs.gpu_bytes(layers), pure.costs.gpu_bytes(layers));
        }
        let native = CanonicalModel {
            metadata: loaded.metadata,
            costs,
            geometry: loaded.geometry.expect("native KV geometry"),
            quantized: false,
        };
        let options = PreviewRequest {
            context_length: Some(8192),
            llama_kv_type: Some("q8_0".into()),
            llama_offload_kqv: Some(true),
            llama_multi_gpu_enabled: Some(true),
            llama_gpu_device_ids: Some(vec![7, 2]),
            ..PreviewRequest::default()
        };
        assert_eq!(
            plan_preview(&native, &options, &hardware(), PreviewSidecars::default()),
            plan_preview(&pure, &options, &hardware(), PreviewSidecars::default())
        );
    }

    #[test]
    fn missing_devices_or_incomplete_geometry_never_produce_a_confident_fit() {
        let header = lettuce_model_hub::parse_gguf_preview(FIXTURE).expect("header");
        let model = canonical_model(&header).expect("model");
        let options = PreviewRequest {
            context_length: Some(8192),
            llama_multi_gpu_enabled: Some(true),
            llama_gpu_device_ids: Some(vec![2, 999]),
            ..PreviewRequest::default()
        };
        assert_eq!(
            preview_label(&plan_preview(
                &model,
                &options,
                &hardware(),
                PreviewSidecars::default()
            )),
            None
        );
        let mut incomplete = header.clone();
        incomplete
            .tensors
            .retain(|tensor| !tensor.name.ends_with("attn_v.weight"));
        assert!(canonical_model(&incomplete).is_none());
        let mut renamed = header;
        renamed.architecture = "future-architecture".into();
        renamed.values = renamed
            .values
            .into_iter()
            .map(|(key, value)| (key.replacen("llama.", "future-architecture.", 1), value))
            .collect();
        assert!(canonical_model(&renamed).is_some());
    }

    #[test]
    fn explicit_full_gpu_and_cpu_choices_are_preserved_on_fragmented_devices() {
        let header = lettuce_model_hub::parse_gguf_preview(FIXTURE).expect("header");
        let model = canonical_model(&header).expect("model");
        let mut device = hardware();
        for gpu in &mut device.devices {
            gpu.memory_free = 1024;
        }
        let mut options = PreviewRequest {
            context_length: Some(8192),
            llama_kv_type: Some("q8_0".into()),
            llama_offload_kqv: Some(true),
            llama_gpu_layers: Some(u32::MAX),
            llama_multi_gpu_enabled: Some(true),
            llama_gpu_device_ids: Some(vec![7, 2]),
            ..PreviewRequest::default()
        };
        let full = plan_preview(&model, &options, &device, PreviewSidecars::default());
        assert_eq!(
            full.estimated_placement
                .as_ref()
                .expect("placement")
                .total_gpu_layers,
            model.metadata.offload_layer_count()
        );
        assert_eq!(
            preview_label(&full),
            Some(dto::RunnabilityLabel::Unrunnable)
        );
        options.llama_gpu_layers = Some(0);
        let cpu = plan_preview(
            &model,
            &options,
            &device,
            PreviewSidecars {
                vram_reserve_bytes: 1024,
                ..PreviewSidecars::default()
            },
        );
        assert_eq!(
            cpu.estimated_placement.expect("placement").total_gpu_layers,
            0
        );
        assert!(cpu.memory_fit.iter().all(|pool| !pool.gpu));
    }

    #[test]
    fn shared_memory_counts_physical_ram_once_and_keeps_gpu_subbudget_separate() {
        let model =
            canonical_model(&lettuce_model_hub::parse_gguf_preview(FIXTURE).expect("header"))
                .expect("model");
        let mut device = hardware();
        device.unified_memory = true;
        let options = PreviewRequest {
            context_length: Some(8192),
            llama_gpu_layers: Some(1),
            llama_offload_kqv: Some(true),
            ..PreviewRequest::default()
        };
        let info = plan_preview(&model, &options, &device, PreviewSidecars::default());
        assert_eq!(info.memory_fit.len(), 1);
        assert_eq!(info.memory_fit[0].capacity_bytes, device.total_memory_bytes);
        assert_eq!(
            info.memory_fit[0].available_bytes,
            device.available_memory_bytes.expect("RAM")
        );
        assert_eq!(preview_label(&info), Some(dto::RunnabilityLabel::Good));
    }

    struct PrefixSource {
        bytes: Vec<u8>,
        lengths: Mutex<Vec<u64>>,
    }
    #[async_trait]
    impl GgufHeaderSource for PrefixSource {
        async fn read_prefix(
            &self,
            _model_id: &str,
            _revision: &str,
            _filename: &str,
            length: u64,
            _token: Option<&SecretValue>,
        ) -> Option<Vec<u8>> {
            self.lengths.lock().expect("lengths").push(length);
            let mut bytes = self.bytes.clone();
            bytes.resize(length as usize, 0);
            bytes.truncate(length as usize);
            Some(bytes)
        }
    }

    #[tokio::test]
    async fn malformed_huge_files_cannot_turn_header_probing_into_a_weight_download() {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let mut bytes = b"GGUF".to_vec();
        bytes.extend(3u32.to_le_bytes());
        bytes.extend(1u64.to_le_bytes());
        bytes.extend(1u64.to_le_bytes());
        bytes.extend((1u64 << 40).to_le_bytes());
        let source = PrefixSource {
            bytes,
            lengths: Mutex::new(Vec::new()),
        };
        let file = dto::HfRunnabilityFile {
            parts: Vec::new(),
            filename: "huge.gguf".into(),
            size: 1u64 << 44,
        };
        assert!(
            browser
                .complete_preview_header(
                    &InMemorySecretStore::default(),
                    &source,
                    RemoteModel {
                        id: "test/model",
                        revision: "main"
                    },
                    &file
                )
                .await
                .expect("unknown metadata")
                .is_none()
        );
        let lengths = source.lengths.lock().expect("lengths");
        assert_eq!(
            lengths.as_slice(),
            &[lettuce_model_hub::GGUF_HEADER_PROBE_BYTES]
        );
        assert!(lengths.iter().all(|length| *length
            <= lettuce_model_hub::GGUF_PREVIEW_METADATA_BYTES
            && *length < file.size));
    }
    struct QuantizedFiles {
        unknown_quant: bool,
        too_short_context: bool,
    }
    #[async_trait]
    impl GgufHeaderSource for QuantizedFiles {
        async fn read_prefix(
            &self,
            _model_id: &str,
            _revision: &str,
            filename: &str,
            length: u64,
            _token: Option<&SecretValue>,
        ) -> Option<Vec<u8>> {
            let quantized = filename.contains("Q8_0");
            let mut bytes = if quantized {
                include_bytes!("../../../lettuce-model-hub/tests/fixtures/preview-llama-q8.gguf")
                    .to_vec()
            } else {
                FIXTURE.to_vec()
            };
            if quantized && self.unknown_quant {
                bytes[..4].copy_from_slice(b"nope");
            }
            if self.too_short_context {
                let key = b"llama.context_length";
                let at = bytes
                    .windows(key.len())
                    .position(|value| value == key)
                    .expect("context metadata")
                    + key.len()
                    + 4;
                bytes[at..at + 4].copy_from_slice(&4096u32.to_le_bytes());
            }
            bytes.truncate(length as usize);
            Some(bytes)
        }
    }

    fn recommendation_request(files: &[&str]) -> dto::HfRecommendationRequest {
        dto::HfRecommendationRequest {
            model_id: "test/model".into(),
            revision: Some("main".into()),
            files: files
                .iter()
                .map(|filename| dto::HfRunnabilityFile {
                    parts: Vec::new(),
                    filename: (*filename).into(),
                    size: if filename.contains("Q8_0") {
                        include_bytes!(
                            "../../../lettuce-model-hub/tests/fixtures/preview-llama-q8.gguf"
                        )
                        .len() as u64
                    } else {
                        FIXTURE.len() as u64
                    },
                })
                .collect(),
            ollama_account_id: None,
            sidecar_reserve_bytes: None,
            plan: None,
            gpu_policy: None,
            sidecars: None,
        }
    }

    #[tokio::test]
    async fn automatic_selection_prefers_known_fitting_quantized_weights_at_eight_k_with_q8_cache()
    {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let request = recommendation_request(&["model-F16.gguf", "model-Q8_0.gguf"]);
        let report = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &QuantizedFiles {
                unknown_quant: false,
                too_short_context: false,
            },
            RemoteModel {
                id: "test/model",
                revision: "main",
            },
            &request,
            &hardware(),
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("report");
        assert_eq!(
            report.recommended_filename.as_deref(),
            Some("model-Q8_0.gguf")
        );
        assert!(
            report
                .files
                .iter()
                .all(|file| file.kv_types.len() == BROWSER_KV_TYPES.len())
        );
        assert_eq!(report.recommended_kv_type.as_deref(), Some("q8_0"));
        let model =
            canonical_model(&lettuce_model_hub::parse_gguf_preview(FIXTURE).expect("header"))
                .expect("geometry");
        assert_eq!(
            report.files[0]
                .kv_types
                .iter()
                .find(|kv| kv.kv_type == "q8_0")
                .expect("Q8")
                .cache_bytes,
            model.geometry.total_bytes(
                8192,
                512,
                KvCacheTypes::from_settings(Some("q8_0"), None, None)
            )
        );
    }

    #[tokio::test]
    async fn automatic_selection_never_falls_back_to_float_weights_when_a_quantized_variant_is_unknown()
     {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let request = recommendation_request(&["model-F16.gguf", "model-Q8_0.gguf"]);
        let report = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &QuantizedFiles {
                unknown_quant: true,
                too_short_context: false,
            },
            RemoteModel {
                id: "test/model",
                revision: "main",
            },
            &request,
            &hardware(),
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("report");
        assert_eq!(report.recommended_filename, None);
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].filename, "model-Q8_0.gguf");
        assert_eq!(report.files[0].label, None);
    }

    #[tokio::test]
    async fn float_only_models_are_allowed_but_insufficient_eight_k_context_or_memory_is_not_recommended()
     {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let request = recommendation_request(&["model-F16.gguf"]);
        let source = QuantizedFiles {
            unknown_quant: false,
            too_short_context: false,
        };
        let report = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &source,
            RemoteModel {
                id: "test/model",
                revision: "main",
            },
            &request,
            &hardware(),
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("report");
        assert_eq!(
            report.recommended_filename.as_deref(),
            Some("model-F16.gguf")
        );
        let short = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &QuantizedFiles {
                unknown_quant: false,
                too_short_context: true,
            },
            RemoteModel {
                id: "test/model",
                revision: "main",
            },
            &request,
            &hardware(),
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("report");
        assert_eq!(short.recommended_filename, None);
        let mut insufficient = hardware();
        insufficient.available_memory_bytes = Some(1);
        insufficient.available_vram_bytes = Some(1);
        for gpu in &mut insufficient.devices {
            gpu.memory_free = 1;
        }
        let report = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &source,
            RemoteModel {
                id: "test/model",
                revision: "main",
            },
            &request,
            &insufficient,
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("report");
        assert_eq!(report.recommended_filename, None);
    }

    #[test]
    fn manual_placement_cannot_borrow_another_devices_free_memory() {
        let model =
            canonical_model(&lettuce_model_hub::parse_gguf_preview(FIXTURE).expect("header"))
                .expect("model");
        let mut device = hardware();
        device.devices[0].memory_free = 1024;
        let options = PreviewRequest {
            context_length: Some(8192),
            llama_kv_type: Some("q8_0".into()),
            llama_offload_kqv: Some(true),
            llama_multi_gpu_enabled: Some(true),
            llama_gpu_device_ids: Some(vec![7, 2]),
            llama_gpu_distribution_mode: Some("manual".into()),
            llama_gpu_manual_layers: Some(vec![
                lettuce_local_llm::context_preview::GpuLayerAssignment {
                    device_id: 2,
                    layers: model.metadata.offload_layer_count(),
                },
                lettuce_local_llm::context_preview::GpuLayerAssignment {
                    device_id: 7,
                    layers: 0,
                },
            ]),
            ..PreviewRequest::default()
        };
        let info = plan_preview(&model, &options, &device, PreviewSidecars::default());
        assert_eq!(
            preview_label(&info),
            Some(dto::RunnabilityLabel::Unrunnable)
        );
        assert_eq!(
            info.estimated_placement
                .expect("placement")
                .per_device_layers,
            vec![0, model.metadata.offload_layer_count()]
        );
    }

    #[test]
    fn huge_alignment_cannot_allocate_metadata_padding() {
        let mut header = lettuce_model_hub::parse_gguf_preview(FIXTURE).expect("header");
        header.tensor_data_offset = 1u64 << 40;
        assert!(complete_model(FIXTURE, &header).is_none());
    }
    #[test]
    fn structurally_plausible_but_runtime_rejected_models_stay_unknown_on_desktop() {
        let mut bytes = FIXTURE.to_vec();
        for index in 0..bytes.len().saturating_sub(4) {
            if &bytes[index..index + 5] == b"llama" {
                bytes[index..index + 5].copy_from_slice(b"xxxxx");
            }
        }
        let header =
            lettuce_model_hub::parse_gguf_preview(&bytes).expect("structurally valid GGUF");
        assert!(canonical_model(&header).is_some());
        assert!(complete_model(&bytes, &header).is_none());
    }

    #[test]
    fn raw_unequal_three_device_split_is_validated_once_as_cumulative_boundaries() {
        assert!(lettuce_local_llm::offload::multi_gpu_placement_fits(
            &[10, 20, 30],
            3,
            &[0.2, 0.3, 0.5],
            &[10, 20, 30]
        ));
        assert!(!lettuce_local_llm::offload::multi_gpu_placement_fits(
            &[10, 20, 30],
            3,
            &[0.2, 0.3, 0.5],
            &[10, 0, 60]
        ));
    }

    #[test]
    fn binary_tensor_storage_matches_the_linked_native_decoder() {
        for kind in 0..43 {
            assert_eq!(
                lettuce_model_hub::gguf_tensor_storage(kind),
                lettuce_local_llm::llama::gguf_tensor_storage(kind),
                "GGUF tensor type {kind}"
            );
        }
    }

    #[tokio::test]
    async fn immutable_metadata_is_shared_but_mutable_main_is_not_cached() {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let source = PrefixSource {
            bytes: FIXTURE.to_vec(),
            lengths: Mutex::new(Vec::new()),
        };
        let file = dto::HfRunnabilityFile {
            parts: Vec::new(),
            filename: "model-F16.gguf".into(),
            size: FIXTURE.len() as u64,
        };
        let revision = "0123456789012345678901234567890123456789";
        let model = RemoteModel {
            id: "test/model",
            revision,
        };
        let first = browser
            .canonical_file(&InMemorySecretStore::default(), &source, model, &file)
            .await
            .expect("lookup")
            .expect("metadata");
        let second = browser
            .canonical_file(&InMemorySecretStore::default(), &source, model, &file)
            .await
            .expect("lookup")
            .expect("metadata");
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert_eq!(source.lengths.lock().expect("requests").len(), 1);
        let model = RemoteModel {
            id: "test/model",
            revision: "main",
        };
        let first = browser
            .canonical_file(&InMemorySecretStore::default(), &source, model, &file)
            .await
            .expect("lookup")
            .expect("metadata");
        let second = browser
            .canonical_file(&InMemorySecretStore::default(), &source, model, &file)
            .await
            .expect("lookup")
            .expect("metadata");
        assert!(!std::sync::Arc::ptr_eq(&first, &second));
        assert_eq!(source.lengths.lock().expect("requests").len(), 3);
    }
    struct RangeFiles {
        files: std::collections::HashMap<String, Vec<u8>>,
        requests: Mutex<Vec<(String, u64, u64)>>,
    }

    #[async_trait]
    impl GgufHeaderSource for RangeFiles {
        async fn read_prefix(
            &self,
            model_id: &str,
            revision: &str,
            filename: &str,
            length: u64,
            token: Option<&SecretValue>,
        ) -> Option<Vec<u8>> {
            self.read_range(model_id, revision, filename, (0, length), u64::MAX, token)
                .await
        }
        async fn read_range(
            &self,
            _model_id: &str,
            _revision: &str,
            filename: &str,
            range: (u64, u64),
            _expected_size: u64,
            _token: Option<&SecretValue>,
        ) -> Option<Vec<u8>> {
            self.requests
                .lock()
                .expect("requests")
                .push((filename.to_owned(), range.0, range.1));
            let bytes = self.files.get(filename)?;
            let end = usize::try_from(range.0.checked_add(range.1)?)
                .ok()?
                .min(bytes.len());
            Some(bytes.get(usize::try_from(range.0).ok()?..end)?.to_vec())
        }
    }

    #[tokio::test]
    async fn automatic_checks_stop_at_the_first_fit_and_selected_settings_reuse_only_that_file() {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let bytes =
            include_bytes!("../../../lettuce-model-hub/tests/fixtures/preview-llama-q8.gguf")
                .to_vec();
        let names = ["model-Q5_K_M.gguf", "model-Q8_0.gguf", "model-F16.gguf"];
        let source = RangeFiles {
            files: names
                .iter()
                .map(|name| ((*name).to_owned(), bytes.clone()))
                .collect(),
            requests: Mutex::new(Vec::new()),
        };
        let mut request = recommendation_request(&names);
        for file in &mut request.files {
            file.size = bytes.len() as u64;
        }
        request.revision = Some("a".repeat(40));
        let model = RemoteModel {
            id: "test/model",
            revision: request.revision.as_deref().expect("revision"),
        };
        let report = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &source,
            model,
            &request,
            &hardware(),
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("report");
        assert_eq!(
            report.recommended_filename.as_deref(),
            Some("model-Q8_0.gguf")
        );
        assert_eq!(source.requests.lock().expect("requests").len(), 1);
        request.plan = Some(dto::HfPlanChoice {
            filename: "model-Q5_K_M.gguf".to_owned(),
            context_length: 8192,
            kv_type: "q8_0".to_owned(),
            model_offload: dto::HfModelOffload::Auto,
            kv_placement: dto::HfKvPlacement::Auto,
        });
        let selected = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &source,
            model,
            &request,
            &hardware(),
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("selected");
        assert_eq!(selected.files.len(), 1);
        assert_eq!(selected.files[0].filename, "model-Q5_K_M.gguf");
        assert_eq!(selected.upgrade.as_deref(), Some("model-F16.gguf"));
        request.plan.as_mut().expect("choice").context_length = 16384;
        let _ = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &source,
            model,
            &request,
            &hardware(),
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("cached");
        let calls = source.requests.lock().expect("requests");
        assert_eq!(calls.len(), 3);
        assert_eq!(
            calls.iter().map(|call| call.0.as_str()).collect::<Vec<_>>(),
            ["model-Q8_0.gguf", "model-Q5_K_M.gguf", "model-F16.gguf"]
        );
    }

    #[tokio::test]
    async fn unknown_highest_candidate_stops_without_scanning_lower_variants() {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let source = RangeFiles {
            files: std::collections::HashMap::new(),
            requests: Mutex::new(Vec::new()),
        };
        let request =
            recommendation_request(&["model-Q5_K_M.gguf", "model-Q8_0.gguf", "model-F16.gguf"]);
        let report = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &source,
            RemoteModel {
                id: "test/model",
                revision: "main",
            },
            &request,
            &hardware(),
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("unknown");
        assert_eq!(report.recommended_filename, None);
        let calls = source.requests.lock().expect("requests");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "model-Q8_0.gguf");
    }

    #[tokio::test]
    async fn metadata_larger_than_legacy_ten_mib_is_read_additively_without_repeating_prefix_bytes()
    {
        let mut bytes = FIXTURE.to_vec();
        let entries = u64::from_le_bytes(bytes[16..24].try_into().expect("entries"));
        bytes[16..24].copy_from_slice(&(entries + 1).to_le_bytes());
        let mut extra = Vec::new();
        let key = b"general.description";
        extra.extend((key.len() as u64).to_le_bytes());
        extra.extend(key);
        extra.extend(8u32.to_le_bytes());
        let length = 11 * 1024 * 1024 + (32 - (extra.len() + 8) % 32) % 32;
        extra.extend((length as u64).to_le_bytes());
        extra.resize(extra.len() + length, b'x');
        bytes.splice(24..24, extra);
        let filename = "model-F16.gguf";
        let size = bytes.len() as u64;
        let source = RangeFiles {
            files: [(filename.to_owned(), bytes)].into(),
            requests: Mutex::new(Vec::new()),
        };
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let file = dto::HfRunnabilityFile {
            filename: filename.to_owned(),
            size,
            parts: Vec::new(),
        };
        let header = browser
            .complete_preview_header(
                &InMemorySecretStore::default(),
                &source,
                RemoteModel {
                    id: "test/model",
                    revision: "main",
                },
                &file,
            )
            .await
            .expect("header result")
            .expect("complete metadata");
        assert!(header.1.tensor_data_offset > 10 * 1024 * 1024);
        let requests = source.requests.lock().expect("requests");
        assert!(requests.len() > 1);
        let mut end = 0;
        for (_, offset, length) in requests.iter() {
            assert_eq!(*offset, end);
            end += length;
        }
        assert!(end <= lettuce_model_hub::GGUF_PREVIEW_METADATA_BYTES);
        assert!(end < size);
    }

    #[tokio::test]
    async fn split_preview_combines_both_indexes_and_matches_editor_metadata_only_planning() {
        let first = "preview-llama-Q8_0-00001-of-00002.gguf";
        let second = "preview-llama-Q8_0-00002-of-00002.gguf";
        let a = include_bytes!(
            "../../../lettuce-model-hub/tests/fixtures/preview-llama-Q8_0-00001-of-00002.gguf"
        )
        .to_vec();
        let b = include_bytes!(
            "../../../lettuce-model-hub/tests/fixtures/preview-llama-Q8_0-00002-of-00002.gguf"
        )
        .to_vec();
        let parts = vec![
            dto::HfFilePart {
                filename: first.to_owned(),
                size: a.len() as u64,
            },
            dto::HfFilePart {
                filename: second.to_owned(),
                size: b.len() as u64,
            },
        ];
        let source = RangeFiles {
            files: [(first.to_owned(), a), (second.to_owned(), b)].into(),
            requests: Mutex::new(Vec::new()),
        };
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let file = dto::HfRunnabilityFile {
            filename: first.to_owned(),
            size: parts.iter().map(|part| part.size).sum(),
            parts,
        };
        let model = browser
            .canonical_file(
                &InMemorySecretStore::default(),
                &source,
                RemoteModel {
                    id: "test/model",
                    revision: "main",
                },
                &file,
            )
            .await
            .expect("read")
            .expect("metadata");
        let model = model.model.as_ref().expect("native split model");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../lettuce-model-hub/tests/fixtures")
            .join(first);
        let editor = lettuce_local_llm::llama::load_model_preview(path.to_str().expect("path"))
            .expect("native editor adapter");
        assert_eq!(model.costs, editor.costs.expect("all shard costs"));
        assert_eq!(
            model.metadata.model_size_bytes,
            editor.metadata.model_size_bytes
        );
        let native = CanonicalModel {
            metadata: editor.metadata,
            costs: model.costs.clone(),
            geometry: editor.geometry.expect("geometry"),
            quantized: true,
        };
        let options = PreviewRequest {
            context_length: Some(8192),
            llama_kv_type: Some("q8_0".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            plan_preview(model, &options, &hardware(), PreviewSidecars::default()).memory_fit,
            plan_preview(&native, &options, &hardware(), PreviewSidecars::default()).memory_fit
        );
        assert_eq!(source.requests.lock().expect("requests").len(), 2);
        let incomplete = dto::HfRunnabilityFile {
            parts: Vec::new(),
            ..file
        };
        assert!(
            browser
                .canonical_file(
                    &InMemorySecretStore::default(),
                    &source,
                    RemoteModel {
                        id: "test/model",
                        revision: "main"
                    },
                    &incomplete
                )
                .await
                .expect("incomplete read")
                .is_none()
        );
    }
    #[tokio::test]
    async fn automatic_probe_priority_defers_oversized_q8_without_claiming_it_cannot_run() {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let bytes =
            include_bytes!("../../../lettuce-model-hub/tests/fixtures/preview-llama-q8.gguf")
                .to_vec();
        let source = RangeFiles {
            files: [
                ("model-Q8_0.gguf".to_owned(), bytes.clone()),
                ("model-Q4_K_M.gguf".to_owned(), bytes),
            ]
            .into(),
            requests: Mutex::new(Vec::new()),
        };
        let mut request = recommendation_request(&["model-Q8_0.gguf", "model-Q4_K_M.gguf"]);
        request.files[0].size = 29_047_086_048;
        request.files[1].size = 14_250_000_000;
        let mut device = hardware();
        device.available_memory_bytes = Some(9_300_000_000);
        device.devices[0].memory_free = 6_000_000_000;
        device.devices[1].memory_free = 0;
        device.available_vram_bytes = Some(6_000_000_000);
        let report = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &source,
            RemoteModel {
                id: "test/model",
                revision: "main",
            },
            &request,
            &device,
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("recommendation");
        assert_eq!(
            report.recommended_filename.as_deref(),
            Some("model-Q4_K_M.gguf")
        );
        assert_eq!(
            source
                .requests
                .lock()
                .expect("requests")
                .iter()
                .map(|call| call.0.as_str())
                .collect::<Vec<_>>(),
            ["model-Q4_K_M.gguf"]
        );
        assert!(
            !report
                .files
                .iter()
                .any(|file| file.filename == "model-Q8_0.gguf")
        );
        request.files.remove(1);
        let before = source.requests.lock().expect("requests").len();
        let unavailable = canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &source,
            RemoteModel {
                id: "test/model",
                revision: "main",
            },
            &request,
            &device,
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("no plausible candidate");
        assert_eq!(unavailable.recommended_filename, None);
        assert!(unavailable.files.is_empty());
        assert_eq!(source.requests.lock().expect("requests").len(), before);
    }

    #[test]
    fn automatic_probe_budget_counts_uma_once_and_only_the_selected_physical_pools() {
        let mut device = hardware();
        device.available_memory_bytes = Some(9_300_000_000);
        device.devices[0].memory_free = 6_000_000_000;
        device.devices[1].memory_free = 12_000_000_000;
        let mut options = PreviewRequest {
            llama_single_gpu_device_id: Some(device.devices[0].index),
            ..Default::default()
        };
        assert_eq!(
            automatic_probe_budget(&options, &device),
            Some(15_300_000_000)
        );
        options.llama_gpu_layers = Some(0);
        assert_eq!(
            automatic_probe_budget(&options, &device),
            Some(9_300_000_000)
        );
        options.llama_gpu_layers = None;
        device.unified_memory = true;
        assert_eq!(
            automatic_probe_budget(&options, &device),
            Some(9_300_000_000)
        );
        device.unified_memory = false;
        options.llama_single_gpu_device_id = None;
        options.llama_multi_gpu_enabled = Some(true);
        options.llama_gpu_device_ids = Some(device.devices.iter().map(|gpu| gpu.index).collect());
        assert_eq!(
            automatic_probe_budget(&options, &device),
            Some(27_300_000_000)
        );
        options.llama_gpu_device_ids = Some(vec![999]);
        assert_eq!(automatic_probe_budget(&options, &device), None);
        device.available_memory_bytes = None;
        assert_eq!(automatic_probe_budget(&options, &device), None);
    }
    #[tokio::test]
    async fn automatic_cache_uses_q8_then_q6_but_manual_cache_controls_fit() {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let source = QuantizedFiles {
            unknown_quant: false,
            too_short_context: false,
        };
        let mut request = recommendation_request(&["model-Q8_0.gguf"]);
        let remote = RemoteModel {
            id: "test/model",
            revision: "main",
        };
        let secrets = InMemorySecretStore::default();
        let parsed = browser
            .canonical_file(&secrets, &source, remote, &request.files[0])
            .await
            .expect("metadata")
            .expect("header");
        let model = parsed.model.as_ref().expect("native model");
        let mut device = hardware();
        device.supports_gpu_offload = false;
        device.available_vram_bytes = None;
        device.devices.clear();
        let defaults = lettuce_models::LlamaCppSettings::default();
        let mut options = preview_options(&preview_policy(&defaults), None, &defaults);
        options.llama_gpu_layers = Some(0);
        options.llama_offload_kqv = Some(false);
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            options.flash_attention =
                Some(lettuce_local_llm::context_info::flash_attention_policy());
        }
        let minimum_ram = |kv: &str| {
            let mut probe = device.clone();
            let mut options = options.clone();
            options.llama_kv_type = Some(kv.to_owned());
            let mut low = 0;
            let mut high = device.available_memory_bytes.expect("RAM");
            while low < high {
                let mid = low + (high - low) / 2;
                probe.available_memory_bytes = Some(mid);
                if preview_label(&plan_preview(
                    model,
                    &options,
                    &probe,
                    PreviewSidecars::default(),
                ))
                .is_some_and(|label| label != dto::RunnabilityLabel::Unrunnable)
                {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            low
        };
        let q8_bytes = minimum_ram("q8_0");
        let q6_bytes = minimum_ram("q6_k");
        let q4_bytes = minimum_ram("q4_0");
        assert!(q6_bytes < q8_bytes);
        device.available_memory_bytes = Some(q6_bytes + (q8_bytes - q6_bytes) / 2);
        let initial = canonical_recommendation(
            &browser, &secrets, &source, remote, &request, &device, &defaults,
        )
        .await
        .expect("auto");
        assert_eq!(
            initial.recommended_filename.as_deref(),
            Some("model-Q8_0.gguf")
        );
        assert_eq!(initial.recommended_kv_type.as_deref(), Some("q6_k"));
        device.available_memory_bytes = Some(q4_bytes + (q6_bytes - q4_bytes) / 2);
        let too_small = canonical_recommendation(
            &browser, &secrets, &source, remote, &request, &device, &defaults,
        )
        .await
        .expect("automatic limit");
        assert_eq!(too_small.recommended_filename, None);
        request.plan = Some(dto::HfPlanChoice {
            filename: "model-Q8_0.gguf".to_owned(),
            context_length: 8192,
            kv_type: "f16".to_owned(),
            model_offload: dto::HfModelOffload::Cpu,
            kv_placement: dto::HfKvPlacement::Ram,
        });
        let large = canonical_recommendation(
            &browser, &secrets, &source, remote, &request, &device, &defaults,
        )
        .await
        .expect("f16");
        assert_eq!(
            large.selected.expect("plan").label,
            dto::RunnabilityLabel::Unrunnable
        );
        assert_eq!(large.recommended_filename, None);
        request.plan.as_mut().expect("choice").kv_type = "q4_0".to_owned();
        let small = canonical_recommendation(
            &browser, &secrets, &source, remote, &request, &device, &defaults,
        )
        .await
        .expect("q4");
        assert_ne!(
            small.selected.expect("plan").label,
            dto::RunnabilityLabel::Unrunnable
        );
        assert_eq!(small.recommended_kv_type.as_deref(), Some("q4_0"));
        let caches = &small.files[0].kv_types;
        assert!(
            caches
                .iter()
                .find(|cache| cache.kv_type == "q4_0")
                .expect("q4")
                .cache_bytes
                < caches
                    .iter()
                    .find(|cache| cache.kv_type == "q8_0")
                    .expect("q8")
                    .cache_bytes
        );
    }

    fn pool(gpu: bool, model_bytes: u64, context_bytes: u64) -> dto::HfCanonicalMemoryFit {
        dto::HfCanonicalMemoryFit {
            gpu,
            devices: Vec::new(),
            capacity_bytes: None,
            available_bytes: u64::MAX,
            model_bytes,
            context_bytes,
            working_bytes: 0,
        }
    }

    #[test]
    fn placement_mode_follows_the_model_and_context_share_on_the_gpu() {
        use dto::HfPlanGpuMode::{Cpu, FewLayers, Full, HalfLayers, KvSpill, MostLayers, NearFull};
        let cases = [
            (vec![pool(true, 100, 50)], 8, Full),
            (vec![pool(true, 100, 0)], 8, Full),
            (vec![pool(true, 95, 50), pool(false, 5, 0)], 8, Full),
            (vec![pool(true, 95, 85), pool(false, 5, 15)], 8, NearFull),
            (vec![pool(true, 100, 85), pool(false, 0, 15)], 8, NearFull),
            (vec![pool(true, 100, 50), pool(false, 0, 50)], 8, KvSpill),
            (vec![pool(true, 80, 10), pool(false, 20, 0)], 6, MostLayers),
            (vec![pool(true, 50, 10), pool(false, 50, 0)], 4, HalfLayers),
            (vec![pool(true, 20, 10), pool(false, 80, 0)], 2, FewLayers),
            (vec![pool(false, 100, 50)], 0, Cpu),
            (vec![pool(true, 0, 10), pool(false, 100, 0)], 3, Cpu),
            (Vec::new(), 3, Cpu),
        ];
        for (fit, layers, mode) in cases {
            assert_eq!(classify_gpu_mode(&fit, layers, 8), mode, "{fit:?} {layers}");
        }
    }

    #[test]
    fn speeds_come_from_the_planner_rating_of_the_mode() {
        use dto::HfPlanGpuMode::{Cpu, FewLayers, Full, HalfLayers, KvSpill, MostLayers, NearFull};
        use dto::HfSpeed::{Fast, Medium, Slow};
        for (mode, prefill, generation) in [
            (Full, Fast, Fast),
            (NearFull, Fast, Fast),
            (KvSpill, Medium, Medium),
            (MostLayers, Slow, Medium),
            (HalfLayers, Slow, Medium),
            (FewLayers, Slow, Slow),
            (Cpu, Slow, Slow),
        ] {
            assert_eq!(
                placement_speeds(mode, false),
                (Some(prefill), Some(generation)),
                "{mode:?}"
            );
            assert_eq!(placement_speeds(mode, true), (None, None), "{mode:?}");
        }
    }

    #[test]
    fn unified_memory_keeps_the_classification_and_drops_the_speeds() {
        let model =
            canonical_model(&lettuce_model_hub::parse_gguf_preview(FIXTURE).expect("header"))
                .expect("model");
        let options = PreviewRequest {
            context_length: Some(8192),
            llama_kv_type: Some("q8_0".into()),
            llama_offload_kqv: Some(true),
            ..PreviewRequest::default()
        };
        let policy = dto::HfGpuPolicy::default();
        let plan_on = |unified: bool| {
            let mut device = hardware();
            device.unified_memory = unified;
            let info = plan_preview(&model, &options, &device, PreviewSidecars::default());
            canonical_plan("model.gguf", 8192, &policy, &device, info).expect("plan")
        };
        let discrete = plan_on(false);
        assert!(discrete.prefill_speed.is_some() && discrete.generation_speed.is_some());
        let unified = plan_on(true);
        assert!(unified.unified_memory);
        assert_eq!(
            unified.gpu_mode,
            classify_gpu_mode(
                &unified.memory_fit,
                unified.gpu_layers,
                unified.total_layers
            )
        );
        assert_eq!(
            (unified.prefill_speed, unified.generation_speed),
            (None, None)
        );
    }

    struct ShortCandidate {
        filename: &'static str,
    }
    #[async_trait]
    impl GgufHeaderSource for ShortCandidate {
        async fn read_prefix(
            &self,
            _model_id: &str,
            _revision: &str,
            filename: &str,
            length: u64,
            _token: Option<&SecretValue>,
        ) -> Option<Vec<u8>> {
            let mut bytes = FIXTURE.to_vec();
            if filename == self.filename {
                let key = b"llama.context_length";
                let at = bytes
                    .windows(key.len())
                    .position(|value| value == key)
                    .expect("context metadata")
                    + key.len()
                    + 4;
                bytes[at..at + 4].copy_from_slice(&2048u32.to_le_bytes());
            }
            bytes.truncate(length as usize);
            Some(bytes)
        }
    }

    async fn upgrade_of(
        files: &[(&str, u64)],
        selected: &str,
        device: &PreviewHardware,
    ) -> Option<String> {
        let browser = HuggingFaceBrowser::new(lettuce_network::JsonClient::new().expect("client"));
        let mut request = recommendation_request(&[]);
        request.files = files
            .iter()
            .map(|(filename, size)| dto::HfRunnabilityFile {
                parts: Vec::new(),
                filename: (*filename).into(),
                size: *size,
            })
            .collect();
        request.plan = Some(dto::HfPlanChoice {
            filename: selected.into(),
            context_length: 8192,
            kv_type: "q8_0".into(),
            model_offload: dto::HfModelOffload::Auto,
            kv_placement: dto::HfKvPlacement::Auto,
        });
        canonical_recommendation(
            &browser,
            &InMemorySecretStore::default(),
            &ShortCandidate {
                filename: "model-Q5_K_M.gguf",
            },
            RemoteModel {
                id: "test/model",
                revision: "main",
            },
            &request,
            device,
            &lettuce_models::LlamaCppSettings::default(),
        )
        .await
        .expect("report")
        .upgrade
    }

    fn one_gpu(free: u64) -> PreviewHardware {
        let mut device = hardware();
        device.devices.truncate(1);
        device.devices[0].memory_free = free;
        device.available_vram_bytes = Some(free);
        device
    }

    #[tokio::test]
    async fn a_better_quantization_that_runs_the_same_way_is_suggested() {
        let size = FIXTURE.len() as u64;
        let files = [
            ("model-Q4_K_M.gguf", size),
            ("model-Q3_K_M.gguf", size),
            ("model-Q5_K_M.gguf", size),
            ("model-Q6_K.gguf", size),
        ];
        assert_eq!(
            upgrade_of(&files, "model-Q4_K_M.gguf", &hardware())
                .await
                .as_deref(),
            Some("model-Q6_K.gguf")
        );
        assert_eq!(
            upgrade_of(&files[..3], "model-Q4_K_M.gguf", &hardware())
                .await
                .as_deref(),
            Some("model-Q5_K_M.gguf")
        );
    }

    #[tokio::test]
    async fn no_upgrade_is_suggested_from_high_quality_or_without_a_better_file() {
        let size = FIXTURE.len() as u64;
        assert_eq!(
            upgrade_of(
                &[("model-Q6_K.gguf", size), ("model-F16.gguf", size)],
                "model-Q6_K.gguf",
                &hardware()
            )
            .await,
            None
        );
        assert_eq!(
            upgrade_of(
                &[("model-Q5_K_M.gguf", size), ("model-Q4_K_M.gguf", size)],
                "model-Q5_K_M.gguf",
                &hardware()
            )
            .await,
            None
        );
        assert_eq!(
            upgrade_of(
                &[("model-Q4_K_M.gguf", size), ("model-Q6_K.gguf", 1 << 50)],
                "model-Q4_K_M.gguf",
                &hardware()
            )
            .await,
            None
        );
    }

    #[tokio::test]
    async fn an_upgrade_that_changes_the_placement_is_rejected() {
        let size = FIXTURE.len() as u64;
        let files = [("model-Q4_K_M.gguf", size), ("model-Q5_K_M.gguf", size)];
        assert_eq!(
            upgrade_of(&files, "model-Q4_K_M.gguf", &one_gpu(298_516_480)).await,
            None
        );
        assert_eq!(
            upgrade_of(&files, "model-Q4_K_M.gguf", &one_gpu(1 << 30))
                .await
                .as_deref(),
            Some("model-Q5_K_M.gguf")
        );
    }

    #[test]
    #[ignore = "needs a local GGUF model in LETTUCE_PLAN_MODEL"]
    fn the_header_parser_and_the_native_loader_describe_a_real_model_alike() {
        use std::io::Read;
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .expect("model")
            .take(lettuce_model_hub::GGUF_PREVIEW_METADATA_BYTES)
            .read_to_end(&mut bytes)
            .expect("header");
        let header = lettuce_model_hub::parse_gguf_preview(&bytes).expect("header index");
        let parsed = canonical_model(&header).expect("canonical model");
        let native = lettuce_local_llm::llama::load_model_preview(&path).expect("native preview");
        assert_eq!(native.metadata, parsed.metadata);
        assert_eq!(native.geometry.as_ref(), Some(&parsed.geometry));
        assert_eq!(native.costs.as_ref(), Some(&parsed.costs));
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    #[ignore = "needs a local GGUF model in LETTUCE_PLAN_MODEL"]
    fn llama_cpp_projects_the_same_memory_for_a_metadata_only_copy() {
        use lettuce_local_llm::llama::{ProjectionShape, project_memory};
        use lettuce_local_llm::offload::FlashAttentionPolicy;
        use std::io::{Read, Write};
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let mut bytes = Vec::new();
        let source = std::fs::File::open(&path).expect("model");
        let length = source.metadata().expect("metadata").len();
        source
            .take(lettuce_model_hub::GGUF_PREVIEW_METADATA_BYTES)
            .read_to_end(&mut bytes)
            .expect("header");
        let header = lettuce_model_hub::parse_gguf_preview(&bytes).expect("header index");
        let sparse =
            std::env::temp_dir().join(format!("lettuce-projection-{}.gguf", uuid::Uuid::new_v4()));
        let mut copy = std::fs::File::create(&sparse).expect("copy");
        copy.write_all(&bytes[..header.tensor_data_offset as usize])
            .expect("prefix");
        copy.set_len(length).expect("logical size");
        let shape = |layers| ProjectionShape {
            n_gpu_layers: layers,
            planned_context: 4096,
            n_batch: 512,
            offload_kqv: Some(true),
            kv_types: KvCacheTypes::uniform(Some("q8_0")),
            flash_attention_policy: FlashAttentionPolicy::Auto,
            swa_full: None,
        };
        for layers in [0, 999] {
            let full = project_memory(&path, shape(layers)).expect("full projection");
            let metadata_only = project_memory(sparse.to_str().expect("path"), shape(layers))
                .expect("metadata-only projection");
            assert_eq!(full, metadata_only);
        }
        std::fs::remove_file(sparse).expect("cleanup");
    }

    enum Value {
        U32(u32),
        U32s(Vec<u32>),
        Bools(Vec<bool>),
        Text(&'static str),
    }

    fn gguf_text(out: &mut Vec<u8>, text: &str) {
        out.extend((text.len() as u64).to_le_bytes());
        out.extend(text.as_bytes());
    }

    fn sliding_window_gguf(pattern: Option<Vec<bool>>, shared_kv_layers: Option<u32>) -> Vec<u8> {
        let mut values = vec![
            ("general.architecture", Value::Text("gemma4")),
            ("gemma4.block_count", Value::U32(6)),
            ("gemma4.context_length", Value::U32(8192)),
            ("gemma4.embedding_length", Value::U32(64)),
            ("gemma4.attention.head_count", Value::U32s(vec![4; 6])),
            (
                "gemma4.attention.head_count_kv",
                Value::U32s(vec![2, 2, 1, 2, 2, 1]),
            ),
            ("gemma4.attention.key_length", Value::U32(32)),
            ("gemma4.attention.value_length", Value::U32(32)),
            ("gemma4.attention.key_length_swa", Value::U32(16)),
            ("gemma4.attention.value_length_swa", Value::U32(16)),
            ("gemma4.attention.sliding_window", Value::U32(128)),
        ];
        if let Some(pattern) = pattern {
            values.push((
                "gemma4.attention.sliding_window_pattern",
                Value::Bools(pattern),
            ));
        }
        if let Some(shared) = shared_kv_layers {
            values.push(("gemma4.attention.shared_kv_layers", Value::U32(shared)));
        }
        let mut tensors: Vec<(String, Vec<u64>)> = vec![
            ("token_embd.weight".into(), vec![64, 100]),
            ("per_layer_token_embd.weight".into(), vec![24, 100]),
            ("output_norm.weight".into(), vec![64]),
        ];
        for (index, width) in [32u64, 32, 32, 32, 32, 32].into_iter().enumerate() {
            tensors.push((format!("blk.{index}.attn_k.weight"), vec![64, width]));
            tensors.push((format!("blk.{index}.attn_v.weight"), vec![64, width]));
        }
        let mut out = b"GGUF".to_vec();
        out.extend(3u32.to_le_bytes());
        out.extend((tensors.len() as u64).to_le_bytes());
        out.extend((values.len() as u64).to_le_bytes());
        for (key, value) in &values {
            gguf_text(&mut out, key);
            match value {
                Value::U32(value) => {
                    out.extend(4u32.to_le_bytes());
                    out.extend(value.to_le_bytes());
                }
                Value::U32s(items) => {
                    out.extend(9u32.to_le_bytes());
                    out.extend(4u32.to_le_bytes());
                    out.extend((items.len() as u64).to_le_bytes());
                    items.iter().for_each(|item| out.extend(item.to_le_bytes()));
                }
                Value::Bools(items) => {
                    out.extend(9u32.to_le_bytes());
                    out.extend(7u32.to_le_bytes());
                    out.extend((items.len() as u64).to_le_bytes());
                    out.extend(items.iter().map(|item| u8::from(*item)));
                }
                Value::Text(text) => {
                    out.extend(8u32.to_le_bytes());
                    gguf_text(&mut out, text);
                }
            }
        }
        let mut offset = 0u64;
        for (name, dimensions) in &tensors {
            gguf_text(&mut out, name);
            out.extend((dimensions.len() as u32).to_le_bytes());
            dimensions.iter().for_each(|d| out.extend(d.to_le_bytes()));
            out.extend(1u32.to_le_bytes());
            out.extend(offset.to_le_bytes());
            let bytes = dimensions.iter().product::<u64>() * 2;
            offset += bytes.div_ceil(32) * 32;
        }
        out.resize(out.len().div_ceil(32) * 32 + offset as usize, 0);
        out
    }

    #[test]
    fn sliding_window_and_shared_cache_layers_get_a_verdict_from_the_header() {
        let pattern = vec![true, true, false, true, true, false];
        let bytes = sliding_window_gguf(Some(pattern), Some(2));
        let header = lettuce_model_hub::parse_gguf_preview(&bytes).expect("header");
        assert_eq!(
            header.values["gemma4.attention.sliding_window_pattern"],
            vec![1, 1, 0, 1, 1, 0]
        );
        let model = canonical_model(&header).expect("sliding window model");
        assert_eq!(model.metadata.n_head, 4);
        assert_eq!(model.metadata.n_head_kv, 2);
        assert_eq!(model.metadata.n_embd_head_k, 32);
        let f16 = KvCacheTypes::uniform(Some("f16"));
        let own_cache = 3 * (768 * 2 * (16 + 16) * 2) + 1024 * (32 + 32) * 2;
        assert_eq!(model.geometry.total_bytes(1024, 512, f16), own_cache);
        let windowless = model.geometry.clone().with_swa_full(true);
        assert!(windowless.total_bytes(1024, 512, f16) > own_cache);

        let tensors = |name: &str| {
            header
                .tensors
                .iter()
                .find(|tensor| tensor.name == name)
                .map(|tensor| tensor.bytes)
                .expect("tensor")
        };
        assert_eq!(model.costs.host_copy_bytes(1), tensors("token_embd.weight"));
        let per_layer = tensors("per_layer_token_embd.weight");
        assert_eq!(
            model.costs.gpu_bytes(model.costs.unit_count()),
            header.model_bytes().expect("size") - per_layer
        );
    }

    #[test]
    fn a_sliding_window_without_a_declared_pattern_or_cache_is_unknown() {
        let missing = sliding_window_gguf(None, Some(2));
        let header = lettuce_model_hub::parse_gguf_preview(&missing).expect("header");
        assert!(canonical_model(&header).is_none());
        let short = sliding_window_gguf(Some(vec![true, false]), None);
        let header = lettuce_model_hub::parse_gguf_preview(&short).expect("header");
        assert!(canonical_model(&header).is_none());
        let everything_shared = sliding_window_gguf(Some(vec![true; 6]), Some(6));
        let header = lettuce_model_hub::parse_gguf_preview(&everything_shared).expect("header");
        assert!(canonical_model(&header).is_none());
    }
}
