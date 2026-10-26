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
    pub quantized: bool,
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
            .scalar("attention.sliding_window")
            .is_some_and(|value| value > 0)
        || header
            .scalar("attention.kv_lora_rank")
            .is_some_and(|value| value > 0)
    {
        return None;
    }
    let count = u32::try_from(header.scalar("block_count")?).ok()?;
    let embedding = header.scalar("embedding_length")?;
    let heads = header.scalar("attention.head_count")?;
    let context = u32::try_from(header.scalar("context_length")?).ok()?;
    if count == 0 || embedding == 0 || heads == 0 || context == 0 || embedding % heads != 0 {
        return None;
    }
    let mut layers = Vec::new();
    for index in 0..count as usize {
        let n_head_kv = header
            .layer_value("attention.head_count_kv", index, count as usize)
            .unwrap_or(heads);
        let k = header
            .scalar("attention.key_length")
            .unwrap_or(embedding / heads);
        let v = header
            .scalar("attention.value_length")
            .unwrap_or(embedding / heads);
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
        layers.push(KvLayerGeometry {
            n_head_kv: u32::try_from(n_head_kv).ok()?,
            n_embd_head_k: u32::try_from(k).ok()?,
            n_embd_head_v: u32::try_from(v).ok()?,
            is_swa: false,
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
        n_head: heads,
        n_head_kv: u64::from(first.n_head_kv),
        n_embd_head_k: u64::from(first.n_embd_head_k),
        n_embd_head_v: u64::from(first.n_embd_head_v),
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
        geometry: KvCacheGeometry::new(layers, 0)?,
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
        let Some((bytes, header)) = self
            .complete_preview_header(secrets, source, model, file)
            .await?
        else {
            return Ok(None);
        };
        let native = complete_model(&bytes, &header);
        let quantized = header.tensors.iter().any(|tensor| {
            tensor.dimensions.len() > 1
                && lettuce_model_hub::gguf_tensor_storage(tensor.kind)
                    .is_some_and(|(block, _)| block > 1)
        });
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
            quantized,
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
                lettuce_model_hub::GGUF_HEADER_RETRY_BYTES.min(file.size.saturating_sub(1));
            if budget < 24 {
                return None;
            }
            let mut length = lettuce_model_hub::GGUF_HEADER_PROBE_BYTES.min(budget);
            loop {
                let bytes = source
                    .read_prefix(
                        model.id,
                        model.revision,
                        &file.filename,
                        length,
                        token.as_ref(),
                    )
                    .await?;
                if let Some(header) = lettuce_model_hub::parse_gguf_preview(&bytes) {
                    if header.tensor_data_offset <= lettuce_model_hub::GGUF_HEADER_RETRY_BYTES
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
        llama_kv_type: Some("q8_0".to_owned()),
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
    let geometry = model
        .geometry
        .clone()
        .with_swa_full(options.swa_full.unwrap_or(true));
    preview_context(
        OffloadModel {
            metadata: model.metadata,
            costs: Some(&model.costs),
            geometry: Some(&geometry),
        },
        options,
        hardware,
        sidecars,
    )
}

pub(crate) fn canonical_plan(
    filename: &str,
    context: u32,
    policy: &dto::HfGpuPolicy,
    hardware: &PreviewHardware,
    info: LlamaCppContextInfo,
) -> Option<dto::HfCanonicalPlan> {
    Some(dto::HfCanonicalPlan {
        unified_memory: hardware.unified_memory,
        filename: filename.to_owned(),
        context_length: context,
        label: preview_label(&info)?,
        gpu_layers: info.estimated_placement.as_ref()?.total_gpu_layers,
        total_layers: info.max_gpu_layers?,
        offload_kqv: info
            .estimated_placement
            .as_ref()
            .is_some_and(|placement| placement.total_gpu_layers > 0)
            && info
                .memory_fit
                .iter()
                .any(|pool| (pool.gpu || hardware.unified_memory) && pool.context_bytes > 0),
        memory_fit: info
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
            .collect(),
        gpu_policy: policy.clone(),
        placement: info
            .estimated_placement
            .map(|placement| dto::LlamaPlacement {
                total_gpu_layers: placement.total_gpu_layers,
                per_device_layers: placement.per_device_layers,
            }),
    })
}

fn complete_model(bytes: &[u8], header: &GgufPreviewMetadata) -> Option<CanonicalModel> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if header.tensors.iter().any(|tensor| {
            lettuce_local_llm::llama::gguf_tensor_storage(tensor.kind)
                != lettuce_model_hub::gguf_tensor_storage(tensor.kind)
        }) {
            return None;
        }
        if header.split_count != 1
            || header.tensor_data_offset > lettuce_model_hub::GGUF_HEADER_RETRY_BYTES
        {
            return None;
        }
        let path =
            std::env::temp_dir().join(format!("lettuce-hf-preview-{}.gguf", uuid::Uuid::new_v4()));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .ok()?;
        let mut prefix = bytes
            .get(
                ..usize::try_from(header.tensor_data_offset)
                    .ok()?
                    .min(bytes.len()),
            )?
            .to_vec();
        prefix.resize(usize::try_from(header.tensor_data_offset).ok()?, 0);
        use std::io::Write;
        let extent = header
            .tensors
            .iter()
            .try_fold(header.tensor_data_offset, |end, tensor| {
                header
                    .tensor_data_offset
                    .checked_add(tensor.offset)?
                    .checked_add(tensor.bytes)
                    .map(|next| end.max(next))
            })?;
        let loaded = file
            .write_all(&prefix)
            .ok()
            .and_then(|_| file.set_len(extent).ok())
            .and_then(|_| {
                drop(file);
                lettuce_local_llm::llama::load_model_preview(path.to_str()?).ok()
            });
        let _ = std::fs::remove_file(path);
        if let Some(loaded) = loaded {
            return Some(CanonicalModel {
                metadata: loaded.metadata,
                costs: loaded.costs?,
                geometry: loaded.geometry?,
                quantized: header.tensors.iter().any(|tensor| {
                    tensor.dimensions.len() > 1
                        && lettuce_model_hub::gguf_tensor_storage(tensor.kind)
                            .is_some_and(|(block, _)| block > 1)
                }),
            });
        }
        None
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = bytes;
        canonical_model(header)
    }
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
    let mut quantized_present = false;
    for file in &request.files {
        let parsed = browser.canonical_file(secrets, source, model, file).await?;
        let native = parsed.as_ref().and_then(|parsed| parsed.model.as_ref());
        let actual_quantized = parsed.as_ref().map(|parsed| parsed.quantized);
        let quantization = lettuce_model_hub::extract_quantization(&file.filename);
        let named_float =
            ["F16", "BF16", "F32", "UD-BF16"].contains(&quantization.to_uppercase().as_str());
        quantized_present |= actual_quantized.unwrap_or(!named_float);
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
            continue;
        };
        let info = plan_preview(native, &options, hardware, sidecars);
        let label = preview_label(&info);
        let mut initial_options = options.clone();
        initial_options.context_length = Some(8192);
        let initial = plan_preview(native, &initial_options, hardware, sidecars);
        let initial_fits = native.metadata.max_context_length >= 8192
            && preview_label(&initial)
                .is_some_and(|label| label != dto::RunnabilityLabel::Unrunnable);
        if initial_fits {
            eligible.push((
                file.filename.clone(),
                native.quantized,
                lettuce_model_hub::quant_quality_score(&quantization),
                file.size,
            ));
        }
        let mut optimal_options = options.clone();
        optimal_options.context_length = None;
        let optimal = plan_preview(native, &optimal_options, hardware, sidecars);
        let context = options
            .context_length
            .unwrap_or(8192)
            .min(native.metadata.max_context_length);
        let cache_bytes = info
            .memory_fit
            .iter()
            .map(|pool| pool.context_bytes)
            .fold(0u64, u64::saturating_add);
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
            kv_types: vec![dto::HfCanonicalKv {
                kv_type: "q8_0".to_owned(),
                cache_bytes,
                recommended_context: optimal.recommended_context_length,
            }],
        });
        if request
            .plan
            .as_ref()
            .is_some_and(|choice| choice.filename == file.filename)
        {
            selected = canonical_plan(&file.filename, context, &policy, hardware, info);
        }
    }
    eligible.retain(|(_, quantized, _, _)| !quantized_present || *quantized);
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
        assert!(lengths.iter().all(
            |length| *length <= lettuce_model_hub::GGUF_HEADER_RETRY_BYTES && *length < file.size
        ));
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
                .all(|file| file.kv_types.len() == 1 && file.kv_types[0].kv_type == "q8_0")
        );
        let model =
            canonical_model(&lettuce_model_hub::parse_gguf_preview(FIXTURE).expect("header"))
                .expect("geometry");
        assert_eq!(
            report.files[0].kv_types[0].cache_bytes,
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
        assert_eq!(report.files[1].label, None);
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
}
