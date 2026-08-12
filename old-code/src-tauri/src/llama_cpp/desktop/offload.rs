use super::engine::shared_backend;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::gguf::GgufContext;
use llama_cpp_2::model::{params::LlamaModelParams, LlamaModel};
use llama_cpp_sys_2::llama_flash_attn_type;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug)]
pub(super) struct LlamaModelMetadata {
    pub(super) model_size_bytes: u64,
    pub(super) layer_count: u32,
    pub(super) nextn_layer_count: u32,
    pub(super) max_context_length: u32,
    pub(super) n_embd: u64,
    pub(super) n_head: u64,
    pub(super) n_head_kv: u64,
    pub(super) n_embd_head_k: u64,
    pub(super) n_embd_head_v: u64,
}

impl LlamaModelMetadata {
    pub(super) fn model_layer_count(&self) -> u32 {
        self.layer_count
            .max(1)
            .saturating_add(self.nextn_layer_count)
    }

    pub(super) fn offload_layer_count(&self) -> u32 {
        self.model_layer_count().saturating_add(1)
    }

    pub(super) fn normalize_requested_gpu_layers(&self, requested: u32) -> u32 {
        if requested >= self.layer_count.max(1) {
            self.offload_layer_count()
        } else {
            requested
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct SmartGpuOffloadPlan {
    pub(super) total_layers: u32,
    pub(super) recommended_context: Option<u32>,
    pub(super) planned_context: u32,
    pub(super) estimated_gpu_layers: u32,
    pub(super) candidate_gpu_layers: Vec<u32>,
    pub(super) kqv_vram_reserved: bool,
    pub(super) planning_offload_kqv: Option<bool>,
    pub(super) estimated_kv_bytes: u64,
    pub(super) kv_bytes_per_layer: u64,
    pub(super) estimated_sidecar_vram_reserve_bytes: u64,
    pub(super) estimated_runtime_reserve_bytes: u64,
    pub(super) effective_vram_budget_bytes: u64,
    pub(super) bytes_per_layer: u64,
    pub(super) offload_unit_costs: Vec<u64>,
}

#[derive(Clone, Debug)]
pub(super) struct ModelOffloadCosts {
    unit_bytes: Vec<u64>,
}

impl ModelOffloadCosts {
    pub(super) fn unit_count(&self) -> u32 {
        u32::try_from(self.unit_bytes.len()).unwrap_or(u32::MAX)
    }

    pub(super) fn gpu_bytes(&self, gpu_layers: u32) -> u64 {
        let take = (gpu_layers as usize).min(self.unit_bytes.len());
        self.unit_bytes[self.unit_bytes.len() - take..]
            .iter()
            .fold(0u64, |acc, bytes| acc.saturating_add(*bytes))
    }

    fn combined_units(&self, kv_per_block: &[u64]) -> Vec<u64> {
        let output_index = self.unit_bytes.len().saturating_sub(1);
        self.unit_bytes
            .iter()
            .enumerate()
            .map(|(index, weight)| {
                if index == output_index {
                    *weight
                } else {
                    weight.saturating_add(kv_per_block.get(index).copied().unwrap_or_default())
                }
            })
            .collect()
    }

    fn max_units_within(&self, budget: u64, kv_per_block: &[u64]) -> u32 {
        let mut running = 0u64;
        let mut fitted = 0u32;
        let output_index = self.unit_bytes.len().saturating_sub(1);
        for (offset, index) in (0..self.unit_bytes.len()).rev().enumerate() {
            running = running.saturating_add(self.unit_bytes[index]);
            if index != output_index {
                running =
                    running.saturating_add(kv_per_block.get(index).copied().unwrap_or_default());
            }
            if running > budget {
                break;
            }
            fitted = u32::try_from(offset + 1).unwrap_or(u32::MAX);
        }
        fitted
    }
}

const KV_CELL_PAD: u64 = 256;

#[derive(Clone, Debug)]
pub(super) struct KvCacheGeometry {
    layers: Vec<llama_cpp_2::model::KvLayerGeometry>,
    n_swa: u32,
}

impl KvCacheGeometry {
    fn cells_for_layer(&self, is_swa: bool, planned_context: u32, n_ubatch: u32) -> u64 {
        let base = u64::from(planned_context.max(1));
        if !is_swa || self.n_swa == 0 {
            return base;
        }
        let swa = u64::from(self.n_swa).saturating_add(u64::from(n_ubatch.max(1)));
        let capped = base.min(swa);
        capped
            .div_ceil(KV_CELL_PAD)
            .saturating_mul(KV_CELL_PAD)
            .min(base)
    }

    fn bytes_per_layer(
        &self,
        planned_context: u32,
        n_ubatch: u32,
        llama_kv_type: Option<&str>,
    ) -> Vec<u64> {
        let bytes_per_value = kv_bytes_per_value(llama_kv_type);
        self.layers
            .iter()
            .map(|layer| {
                let cells = self.cells_for_layer(layer.is_swa, planned_context, n_ubatch);
                let per_cell = u64::from(layer.n_head_kv).saturating_mul(
                    u64::from(layer.n_embd_head_k) + u64::from(layer.n_embd_head_v),
                );
                ((cells.saturating_mul(per_cell)) as f64 * bytes_per_value) as u64
            })
            .collect()
    }

    fn total_bytes(&self, planned_context: u32, n_ubatch: u32, llama_kv_type: Option<&str>) -> u64 {
        self.bytes_per_layer(planned_context, n_ubatch, llama_kv_type)
            .into_iter()
            .fold(0u64, |acc, bytes| acc.saturating_add(bytes))
    }

    fn max_context_within(
        &self,
        budget: u64,
        n_ubatch: u32,
        llama_kv_type: Option<&str>,
        max_context: u32,
    ) -> u32 {
        if self.total_bytes(1, n_ubatch, llama_kv_type) > budget {
            return 0;
        }
        let (mut lo, mut hi) = (1u32, max_context.max(1));
        if self.total_bytes(hi, n_ubatch, llama_kv_type) <= budget {
            return hi;
        }
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            if self.total_bytes(mid, n_ubatch, llama_kv_type) <= budget {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }
}

fn parse_kv_cache_type(value: &str) -> Option<llama_cpp_2::context::params::KvCacheType> {
    use llama_cpp_2::context::params::KvCacheType;
    match value.trim().to_ascii_lowercase().as_str() {
        "f32" => Some(KvCacheType::F32),
        "f16" => Some(KvCacheType::F16),
        "q8_1" => Some(KvCacheType::Q8_1),
        "q8_0" => Some(KvCacheType::Q8_0),
        "q6_k" => Some(KvCacheType::Q6_K),
        "q5_k" => Some(KvCacheType::Q5_K),
        "q5_1" => Some(KvCacheType::Q5_1),
        "q5_0" => Some(KvCacheType::Q5_0),
        "q4_k" => Some(KvCacheType::Q4_K),
        "q4_1" => Some(KvCacheType::Q4_1),
        "q4_0" => Some(KvCacheType::Q4_0),
        "q3_k" => Some(KvCacheType::Q3_K),
        "q2_k" => Some(KvCacheType::Q2_K),
        "iq4_nl" => Some(KvCacheType::IQ4_NL),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ComputeProbeKey {
    model_path_hash: u64,
    n_gpu_layers: u32,
    planned_context: u32,
    n_batch: u32,
    offload_kqv: Option<bool>,
    flash_attention_policy: i32,
    kv_type_hash: u64,
}

static COMPUTE_PROBE_CACHE: OnceLock<Mutex<HashMap<ComputeProbeKey, Option<u64>>>> =
    OnceLock::new();

fn compute_probe_cache() -> &'static Mutex<HashMap<ComputeProbeKey, Option<u64>>> {
    COMPUTE_PROBE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn stable_hash(value: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn measure_device_compute_bytes(
    model_path: &str,
    n_gpu_layers: u32,
    planned_context: u32,
    n_batch: u32,
    offload_kqv: Option<bool>,
    llama_kv_type: Option<&str>,
    flash_attention_policy: llama_flash_attn_type,
) -> Option<u64> {
    let key = ComputeProbeKey {
        model_path_hash: stable_hash(model_path),
        n_gpu_layers,
        planned_context,
        n_batch,
        offload_kqv,
        flash_attention_policy,
        kv_type_hash: stable_hash(llama_kv_type.unwrap_or("")),
    };
    if let Some(cached) = compute_probe_cache().lock().ok()?.get(&key).copied() {
        return cached;
    }

    let mut model_params = LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);
    let mut context_params = LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(planned_context.max(1)))
        .with_n_batch(n_batch.max(1))
        .with_n_ubatch(n_batch.max(1))
        .with_flash_attention_policy(flash_attention_policy);
    if let Some(offload) = offload_kqv {
        context_params = context_params.with_offload_kqv(offload);
    }
    if let Some(kv_type) = llama_kv_type.and_then(parse_kv_cache_type) {
        context_params = context_params.with_type_k(kv_type).with_type_v(kv_type);
    }
    model_params = model_params.with_no_alloc(true);

    let measured = model_params
        .project_memory(Path::new(model_path), &context_params)
        .map(|projection| projection.device_compute)
        .filter(|bytes| *bytes > 0);

    if let Ok(mut cache) = compute_probe_cache().lock() {
        cache.insert(key, measured);
    }
    measured
}

fn load_kv_geometry_from_model(model: &LlamaModel) -> Option<KvCacheGeometry> {
    let geometry = model.kv_geometry()?;
    (!geometry.layers.is_empty()).then_some(KvCacheGeometry {
        layers: geometry.layers,
        n_swa: geometry.n_swa,
    })
}

pub(super) fn load_kv_geometry(model_path: &str) -> Option<KvCacheGeometry> {
    if let Some(cached) = kv_geometry_cache().lock().ok()?.get(model_path).cloned() {
        return cached;
    }
    load_model_metadata(model_path).ok()?;
    kv_geometry_cache().lock().ok()?.get(model_path).cloned()?
}

static KV_GEOMETRY_CACHE: OnceLock<Mutex<HashMap<String, Option<KvCacheGeometry>>>> =
    OnceLock::new();

fn kv_geometry_cache() -> &'static Mutex<HashMap<String, Option<KvCacheGeometry>>> {
    KV_GEOMETRY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load_offload_costs_uncached(model_path: &str) -> Option<ModelOffloadCosts> {
    let gguf = GgufContext::from_file(Path::new(model_path))?;
    let mut blocks: BTreeMap<u32, u64> = BTreeMap::new();
    let mut output_bytes = 0u64;
    let mut input_bytes = 0u64;
    let mut has_output_weight = false;

    for index in 0..gguf.n_tensors() {
        let Some(name) = gguf.tensor_name(index) else {
            continue;
        };
        let size = gguf.tensor_size(index);
        if let Some(block) = block_index(name) {
            let entry = blocks.entry(block).or_default();
            *entry = entry.saturating_add(size);
        } else if name.starts_with("token_embd") {
            input_bytes = input_bytes.saturating_add(size);
        } else {
            if name == "output.weight" {
                has_output_weight = true;
            }
            output_bytes = output_bytes.saturating_add(size);
        }
    }

    let n_layer_all = usize::try_from(*blocks.keys().max()? + 1).ok()?;
    if !has_output_weight {
        output_bytes = output_bytes.saturating_add(input_bytes);
    }

    let mut unit_bytes = vec![0u64; n_layer_all + 1];
    for (block, bytes) in blocks {
        if let Some(slot) = unit_bytes.get_mut(block as usize) {
            *slot = bytes;
        }
    }
    unit_bytes[n_layer_all] = output_bytes;

    Some(ModelOffloadCosts { unit_bytes })
}

fn block_index(tensor_name: &str) -> Option<u32> {
    let rest = tensor_name.strip_prefix("blk.")?;
    let (digits, _) = rest.split_once('.')?;
    digits.parse().ok()
}

pub(super) fn load_offload_costs(model_path: &str) -> Option<ModelOffloadCosts> {
    if let Some(costs) = offload_costs_cache().lock().ok()?.get(model_path).cloned() {
        return costs;
    }
    let costs = load_offload_costs_uncached(model_path);
    offload_costs_cache()
        .lock()
        .ok()?
        .insert(model_path.to_string(), costs.clone());
    costs
}

static MODEL_OFFLOAD_COSTS_CACHE: OnceLock<Mutex<HashMap<String, Option<ModelOffloadCosts>>>> =
    OnceLock::new();

fn offload_costs_cache() -> &'static Mutex<HashMap<String, Option<ModelOffloadCosts>>> {
    MODEL_OFFLOAD_COSTS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
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
        Some("f16") | Some("bf16") => 2.0,
        Some("q8_1") => 36.0 / 32.0,
        Some("q8_0") => 34.0 / 32.0,
        Some("q6_k") => 210.0 / 256.0,
        Some("q5_k") => 176.0 / 256.0,
        Some("q5_1") => 24.0 / 32.0,
        Some("q5_0") => 22.0 / 32.0,
        Some("q4_k") => 144.0 / 256.0,
        Some("q4_1") => 20.0 / 32.0,
        Some("q4_0") | Some("iq4_nl") => 18.0 / 32.0,
        Some("q3_k") => 110.0 / 256.0,
        Some("q2_k") => 84.0 / 256.0,
        _ => 2.0,
    }
}

fn estimate_kv_bytes_per_token(
    metadata: &LlamaModelMetadata,
    llama_kv_type: Option<&str>,
) -> Option<u64> {
    let n_layer = u64::from(metadata.layer_count.max(1));
    let n_head_kv = metadata.n_head_kv.max(1);
    let head_bytes = metadata.n_embd_head_k.max(1) + metadata.n_embd_head_v.max(1);
    let bytes_per_value = kv_bytes_per_value(llama_kv_type);
    let bytes = (n_layer as f64) * (n_head_kv as f64) * (head_bytes as f64) * bytes_per_value;
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
    geometry: Option<&KvCacheGeometry>,
    n_ubatch: u32,
    gpu_weight_bytes: u64,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    llama_offload_kqv: Option<bool>,
    llama_kv_type: Option<&str>,
) -> Option<u32> {
    let available_for_ctx = if llama_offload_kqv == Some(true) {
        let vram = available_vram_bytes?;
        let reserve = default_memory_reserve_bytes(vram);
        vram.saturating_sub(reserve.saturating_add(gpu_weight_bytes))
    } else {
        let ram = available_memory_bytes?;
        ram_budget_for_context(metadata, ram)
    };
    if let Some(geometry) = geometry {
        return Some(geometry.max_context_within(
            available_for_ctx,
            n_ubatch,
            llama_kv_type,
            metadata.max_context_length,
        ));
    }
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

    let n_embd = u64::try_from(model.n_embd()).unwrap_or(0).max(1);
    let n_head = u64::from(model.n_head()).max(1);
    let implied_head_dim = (n_embd / n_head).max(1);
    let (n_embd_head_k, n_embd_head_v) = gguf_head_dims(model_path, implied_head_dim);

    if let Ok(mut cache) = kv_geometry_cache().lock() {
        cache.insert(model_path.to_string(), load_kv_geometry_from_model(&model));
    }

    Ok(LlamaModelMetadata {
        model_size_bytes: model.size(),
        layer_count: model.n_layer().max(1),
        nextn_layer_count: model.n_layer_nextn(),
        max_context_length: model.n_ctx_train().max(1),
        n_embd,
        n_head,
        n_head_kv: u64::from(model.n_head_kv()).max(1),
        n_embd_head_k,
        n_embd_head_v,
    })
}

fn gguf_head_dims(model_path: &str, fallback: u64) -> (u64, u64) {
    let Some(gguf) = GgufContext::from_file(Path::new(model_path)) else {
        return (fallback, fallback);
    };
    let arch_idx = gguf.find_key("general.architecture");
    if arch_idx < 0 {
        return (fallback, fallback);
    }
    let Some(arch) = gguf.val_str(arch_idx) else {
        return (fallback, fallback);
    };
    let read = |suffix: &str| -> Option<u64> {
        let idx = gguf.find_key(&format!("{arch}.attention.{suffix}"));
        if idx < 0 {
            return None;
        }
        let value = gguf.val_u32(idx);
        (value > 0).then(|| u64::from(value))
    };
    (
        read("key_length").unwrap_or(fallback),
        read("value_length").unwrap_or(fallback),
    )
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

const ATTENTION_SCORE_BYTES: u64 = 4;
const COMPUTE_BUFFER_SAFETY_FACTOR: u64 = 2;
const COMPUTE_RESERVE_FLOOR_BYTES: u64 = 256 * 1024 * 1024;

pub(super) fn estimate_mtp_gpu_reserve_bytes(
    model_path: &str,
    planned_context: u32,
    n_ubatch: u32,
    llama_kv_type: Option<&str>,
) -> Result<u64, String> {
    let metadata = load_model_metadata(model_path)?;
    let draft_kv_bytes = match load_kv_geometry(model_path) {
        Some(geometry) => geometry.total_bytes(planned_context, n_ubatch, llama_kv_type),
        None => estimate_kv_bytes_per_token(&metadata, llama_kv_type)
            .unwrap_or(0)
            .saturating_mul(u64::from(planned_context.max(1))),
    };
    Ok(metadata.model_size_bytes.saturating_add(draft_kv_bytes))
}

pub(super) fn select_mtp_gpu_device(
    selected_device_ids: &[usize],
    device_free_vram: &[u64],
) -> Option<usize> {
    selected_device_ids
        .iter()
        .copied()
        .zip(device_free_vram.iter().copied())
        .max_by_key(|(_, free)| *free)
        .map(|(device_id, _)| device_id)
}

pub(super) fn reserve_device_vram(
    selected_device_ids: &[usize],
    device_free_vram: &[u64],
    device_id: Option<usize>,
    reserve_bytes: u64,
) -> Vec<u64> {
    let mut adjusted = device_free_vram.to_vec();
    if let Some(position) = device_id.and_then(|device_id| {
        selected_device_ids
            .iter()
            .position(|selected| *selected == device_id)
    }) {
        if let Some(free) = adjusted.get_mut(position) {
            *free = free.saturating_sub(reserve_bytes);
        }
    }
    adjusted
}

fn estimated_runtime_reserve_bytes(
    metadata: &LlamaModelMetadata,
    available_vram_bytes: u64,
    planned_context: u32,
    n_batch: u32,
    flash_attention_policy: llama_flash_attn_type,
) -> u64 {
    let floor = (available_vram_bytes / 20).max(COMPUTE_RESERVE_FLOOR_BYTES);
    // AUTO (-1) means llama.cpp will use flash attention when the backend supports it
    // (always true on CUDA). Only reserve the full attention matrix for the DISABLED case.
    let attention_reserve =
        if flash_attention_policy != llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_DISABLED {
            0
        } else {
            u64::from(planned_context.max(1))
                .saturating_mul(u64::from(n_batch.max(1)))
                .saturating_mul(metadata.n_head.max(1))
                .saturating_mul(ATTENTION_SCORE_BYTES)
                .saturating_mul(COMPUTE_BUFFER_SAFETY_FACTOR)
        };
    floor.saturating_add(attention_reserve)
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
    push_unique(&mut candidates, estimate);
    push_unique(&mut candidates, estimate.saturating_mul(3) / 4);
    push_unique(&mut candidates, estimate / 2);
    push_unique(&mut candidates, estimate / 4);
    push_unique(&mut candidates, 0);
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    candidates
}

pub(super) fn context_bucket_upper(context: u32) -> u32 {
    match context {
        0..=4096 => 4096,
        4097..=8192 => 8192,
        8193..=12288 => 12288,
        12289..=16384 => 16384,
        16385..=24576 => 24576,
        24577..=32768 => 32768,
        32769..=49152 => 49152,
        49153..=65536 => 65536,
        _ => ((context.saturating_add(8191)) / 8192) * 8192,
    }
}

pub(super) fn merge_cached_candidate_layers(
    total_layers: u32,
    cached_gpu_layers: u32,
    heuristic_candidates: &[u32],
) -> Vec<u32> {
    let mut merged = Vec::new();
    let cached = cached_gpu_layers.min(total_layers);
    if cached > 0 {
        push_unique(&mut merged, cached);
        push_unique(&mut merged, cached.saturating_mul(3) / 4);
        push_unique(&mut merged, cached / 2);
        push_unique(&mut merged, cached / 4);
    }
    for candidate in heuristic_candidates {
        push_unique(&mut merged, (*candidate).min(total_layers));
    }
    push_unique(&mut merged, 0);
    merged
}

pub(super) fn model_weight_split_bytes(
    metadata: &LlamaModelMetadata,
    costs: Option<&ModelOffloadCosts>,
    gpu_layers: u32,
) -> (u64, u64) {
    if let Some(costs) = costs {
        let gpu_weight_bytes = costs.gpu_bytes(gpu_layers);
        let cpu_weight_bytes = metadata
            .model_size_bytes
            .saturating_sub(gpu_weight_bytes.min(metadata.model_size_bytes));
        return (cpu_weight_bytes, gpu_weight_bytes);
    }
    let total_layers = metadata.offload_layer_count();
    let clamped_gpu_layers = gpu_layers.min(total_layers);
    let gpu_weight_bytes = metadata
        .model_size_bytes
        .saturating_mul(u64::from(clamped_gpu_layers))
        .checked_div(u64::from(total_layers))
        .unwrap_or(0);
    let cpu_weight_bytes = metadata.model_size_bytes.saturating_sub(gpu_weight_bytes);
    (cpu_weight_bytes, gpu_weight_bytes)
}

pub(super) fn compute_recommended_context_for_gpu_layers(
    metadata: &LlamaModelMetadata,
    costs: Option<&ModelOffloadCosts>,
    geometry: Option<&KvCacheGeometry>,
    n_ubatch: u32,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    gpu_layers: u32,
    llama_offload_kqv: Option<bool>,
    llama_kv_type: Option<&str>,
    sidecar_vram_reserve_bytes: u64,
) -> Option<u32> {
    let (cpu_weight_bytes, gpu_weight_bytes) =
        model_weight_split_bytes(metadata, costs, gpu_layers);
    let available_for_ctx = if llama_offload_kqv == Some(true) {
        let vram = available_vram_bytes?;
        let reserve = default_memory_reserve_bytes(vram);
        vram.saturating_sub(gpu_weight_bytes.saturating_add(reserve))
            .saturating_sub(sidecar_vram_reserve_bytes)
    } else {
        let ram = available_memory_bytes?;
        let reserve = default_memory_reserve_bytes(ram);
        ram.saturating_sub(cpu_weight_bytes.saturating_add(reserve))
    };
    if let Some(geometry) = geometry {
        return Some(geometry.max_context_within(
            available_for_ctx,
            n_ubatch,
            llama_kv_type,
            metadata.max_context_length,
        ));
    }
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

pub(super) fn plan_smart_gpu_offload(
    model_path: &str,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    requested_context: Option<u32>,
    n_batch: u32,
    resolved_offload_kqv: Option<bool>,
    llama_kv_type: Option<&str>,
    flash_attention_policy: llama_flash_attn_type,
    sidecar_vram_reserve_bytes: u64,
    bundled_mtp_draft: bool,
) -> Result<SmartGpuOffloadPlan, String> {
    let metadata = load_model_metadata(model_path)?;
    let costs = load_offload_costs(model_path);
    let total_layers = costs
        .as_ref()
        .map(ModelOffloadCosts::unit_count)
        .unwrap_or_else(|| metadata.offload_layer_count());
    let geometry = load_kv_geometry(model_path);
    let available_vram = available_vram_bytes.unwrap_or(0);
    let effective_vram_budget_bytes = available_vram.saturating_mul(9) / 10;
    let bytes_per_layer = metadata
        .model_size_bytes
        .checked_add(u64::from(metadata.model_layer_count()) - 1)
        .and_then(|bytes| bytes.checked_div(u64::from(metadata.model_layer_count())))
        .unwrap_or(0);
    let kv_bytes_per_token = estimate_kv_bytes_per_token(&metadata, llama_kv_type).unwrap_or(0);
    let planning_offload_kqv = resolved_offload_kqv;
    let kqv_vram_reserved = planning_offload_kqv != Some(false);
    let kv_contexts = if bundled_mtp_draft { 2 } else { 1 };

    let kv_for_context = |planned_context: u32| -> (Vec<u64>, u64) {
        if !kqv_vram_reserved {
            return (Vec::new(), 0);
        }
        let uniform = kv_bytes_per_token
            .saturating_mul(u64::from(planned_context))
            .checked_div(u64::from(metadata.layer_count.max(1)))
            .unwrap_or(0)
            .saturating_mul(kv_contexts);
        let per_block = if let Some(geometry) = geometry.as_ref() {
            geometry
                .bytes_per_layer(planned_context, n_batch, llama_kv_type)
                .into_iter()
                .map(|bytes| bytes.saturating_mul(kv_contexts))
                .collect()
        } else {
            vec![uniform; metadata.layer_count.max(1) as usize]
        };
        (per_block, uniform)
    };

    let layers_for_context =
        |planned_context: u32, kv_per_block: &[u64], measured_compute: Option<u64>| -> (u32, u64) {
            let runtime_reserve = measured_compute.unwrap_or_else(|| {
                estimated_runtime_reserve_bytes(
                    &metadata,
                    available_vram,
                    planned_context,
                    n_batch,
                    flash_attention_policy,
                )
            });
            let available_base = effective_vram_budget_bytes
                .saturating_sub(runtime_reserve)
                .saturating_sub(sidecar_vram_reserve_bytes);
            let layers = match costs.as_ref() {
                Some(costs) => costs
                    .max_units_within(available_base, kv_per_block)
                    .min(total_layers),
                None => {
                    let average_kv = if kv_per_block.is_empty() {
                        0
                    } else {
                        kv_per_block.iter().sum::<u64>() / kv_per_block.len() as u64
                    };
                    let effective = bytes_per_layer.saturating_add(average_kv);
                    if available_base == 0 || effective == 0 {
                        0
                    } else {
                        u32::try_from((available_base / effective).min(u64::from(total_layers)))
                            .unwrap_or(total_layers)
                            .min(total_layers)
                    }
                }
            };
            (layers, runtime_reserve)
        };

    let mut recommended_context = compute_recommended_context(
        &metadata,
        geometry.as_ref(),
        n_batch,
        0,
        available_memory_bytes,
        available_vram_bytes,
        resolved_offload_kqv,
        llama_kv_type,
    );
    let mut planned_context = requested_context
        .or(recommended_context)
        .unwrap_or(metadata.max_context_length)
        .clamp(1, metadata.max_context_length);
    let (mut kv_per_block, mut kv_bytes_per_layer) = kv_for_context(planned_context);
    let (mut estimated_gpu_layers, mut estimated_runtime_reserve_bytes) =
        layers_for_context(planned_context, &kv_per_block, None);

    let measured_compute = measure_device_compute_bytes(
        model_path,
        estimated_gpu_layers.max(1),
        planned_context,
        n_batch,
        resolved_offload_kqv,
        llama_kv_type,
        flash_attention_policy,
    );
    if measured_compute.is_some() {
        let refreshed = layers_for_context(planned_context, &kv_per_block, measured_compute);
        estimated_gpu_layers = refreshed.0;
        estimated_runtime_reserve_bytes = refreshed.1;
    }

    if requested_context.is_none() {
        let resident_weights = costs
            .as_ref()
            .map(|costs| costs.gpu_bytes(estimated_gpu_layers))
            .unwrap_or_else(|| model_weight_split_bytes(&metadata, None, estimated_gpu_layers).1);
        recommended_context = compute_recommended_context(
            &metadata,
            geometry.as_ref(),
            n_batch,
            resident_weights,
            available_memory_bytes,
            available_vram_bytes,
            resolved_offload_kqv,
            llama_kv_type,
        );
        if let Some(recommended) = recommended_context.filter(|value| *value > 0) {
            planned_context = recommended.clamp(1, metadata.max_context_length);
            let refreshed = kv_for_context(planned_context);
            kv_per_block = refreshed.0;
            kv_bytes_per_layer = refreshed.1;
            let refreshed = layers_for_context(planned_context, &kv_per_block, measured_compute);
            estimated_gpu_layers = refreshed.0;
            estimated_runtime_reserve_bytes = refreshed.1;
        }
    }

    let estimated_kv_bytes = if estimated_gpu_layers == 0 {
        0
    } else {
        let unit_count = total_layers as usize;
        let first_offloaded = unit_count.saturating_sub(estimated_gpu_layers as usize);
        let output_index = unit_count.saturating_sub(1);
        (first_offloaded..output_index)
            .filter_map(|index| kv_per_block.get(index))
            .fold(0u64, |acc, bytes| acc.saturating_add(*bytes))
    };

    let offload_unit_costs = costs
        .as_ref()
        .map(|costs| costs.combined_units(&kv_per_block))
        .unwrap_or_default();

    Ok(SmartGpuOffloadPlan {
        total_layers,
        recommended_context,
        planned_context,
        estimated_gpu_layers,
        candidate_gpu_layers: candidate_gpu_layers(total_layers, estimated_gpu_layers),
        kqv_vram_reserved,
        planning_offload_kqv,
        estimated_kv_bytes,
        kv_bytes_per_layer,
        estimated_sidecar_vram_reserve_bytes: sidecar_vram_reserve_bytes,
        estimated_runtime_reserve_bytes,
        effective_vram_budget_bytes,
        bytes_per_layer,
        offload_unit_costs,
    })
}

fn offloaded_units(unit_costs: &[u64], total: u32) -> Vec<u64> {
    let take = (total as usize).min(unit_costs.len());
    unit_costs[unit_costs.len() - take..].to_vec()
}

fn distribution_fits(offloaded: &[u64], split: &[f32], device_free: &[u64]) -> bool {
    if offloaded.is_empty() {
        return true;
    }
    let mut used = vec![0u64; device_free.len()];
    for (position, cost) in offloaded.iter().enumerate() {
        let fraction = position as f32 / offloaded.len() as f32;
        let device = split
            .iter()
            .position(|bound| fraction < *bound)
            .unwrap_or(device_free.len().saturating_sub(1));
        if let Some(slot) = used.get_mut(device) {
            *slot = slot.saturating_add(*cost);
        }
    }
    used.iter()
        .zip(device_free)
        .all(|(used, free)| used <= free)
}

fn largest_total_that_fits(
    unit_costs: &[u64],
    auto_total: u32,
    split: &[f32],
    device_free: &[u64],
) -> u32 {
    let mut cumulative = Vec::with_capacity(split.len());
    let mut running = 0.0f32;
    for weight in split {
        running += *weight;
        cumulative.push(running);
    }
    for total in (0..=auto_total).rev() {
        if distribution_fits(
            &offloaded_units(unit_costs, total),
            &cumulative,
            device_free,
        ) {
            return total;
        }
    }
    0
}

#[derive(Debug, Clone, Default)]
pub(super) struct MultiGpuDistribution {
    pub(super) n_gpu_layers: u32,
    pub(super) tensor_split: Vec<f32>,
    pub(super) main_gpu: Option<i32>,
    pub(super) per_device_layers: Vec<u32>,
}

fn normalize_weights(weights: &[f32]) -> Vec<f32> {
    let n = weights.len();
    if n == 0 {
        return Vec::new();
    }
    let sum: f32 = weights.iter().copied().filter(|w| *w > 0.0).sum();
    if sum <= 0.0 {
        return vec![1.0 / n as f32; n];
    }
    weights.iter().map(|w| w.max(0.0) / sum).collect()
}

/// Split `total` whole layers across devices following `weights`, summing exactly
/// to `total` (largest-remainder method). Used for the UI placement estimate.
fn distribute_by_weights(total: u32, weights: &[f32]) -> Vec<u32> {
    let n = weights.len();
    if n == 0 {
        return Vec::new();
    }
    if total == 0 {
        return vec![0u32; n];
    }
    let sum: f32 = weights.iter().copied().filter(|w| *w > 0.0).sum();
    let raw: Vec<f32> = if sum <= 0.0 {
        vec![total as f32 / n as f32; n]
    } else {
        weights
            .iter()
            .map(|w| (w.max(0.0) / sum) * total as f32)
            .collect()
    };
    let mut out: Vec<u32> = raw.iter().map(|r| r.floor() as u32).collect();
    let assigned: u32 = out.iter().copied().sum();
    let mut remainder = total.saturating_sub(assigned);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| {
        let fa = raw[*a] - raw[*a].floor();
        let fb = raw[*b] - raw[*b].floor();
        fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut i = 0;
    while remainder > 0 {
        let idx = order[i % n];
        out[idx] += 1;
        remainder -= 1;
        i += 1;
    }
    out
}

/// Translate a distribution strategy into concrete llama.cpp load parameters.
/// `device_free_vram` and `manual` are aligned to the selected-device order.
pub(super) fn plan_multi_gpu_distribution(
    mode: &str,
    device_free_vram: &[u64],
    total_layers: u32,
    bytes_per_layer: u64,
    kv_bytes_per_layer: u64,
    smart_total_estimate: u32,
    manual: Option<&[u32]>,
    priority_limit_bytes: Option<u64>,
    unit_costs: Option<&[u64]>,
) -> MultiGpuDistribution {
    let n = device_free_vram.len();
    if n == 0 {
        return MultiGpuDistribution::default();
    }
    let auto_total = smart_total_estimate.min(total_layers);
    let exact = unit_costs.filter(|costs| !costs.is_empty());

    match mode {
        "manual" => {
            let counts: Vec<u32> = (0..n)
                .map(|i| manual.and_then(|m| m.get(i).copied()).unwrap_or(0))
                .collect();
            let total: u32 = counts.iter().copied().sum::<u32>().min(total_layers);
            let weights: Vec<f32> = counts.iter().map(|c| *c as f32).collect();
            MultiGpuDistribution {
                n_gpu_layers: total,
                tensor_split: if total > 0 {
                    normalize_weights(&weights)
                } else {
                    Vec::new()
                },
                main_gpu: None,
                per_device_layers: counts,
            }
        }
        "priority" => {
            let effective_per_layer = bytes_per_layer.saturating_add(kv_bytes_per_layer);
            let mut remaining = auto_total;
            let mut per_device = vec![0u32; n];
            let offloaded = exact.map(|costs| offloaded_units(costs, auto_total));
            let mut cursor = 0usize;
            for (i, free) in device_free_vram.iter().enumerate() {
                if remaining == 0 {
                    break;
                }
                let budget = if i == 0 {
                    priority_limit_bytes
                        .map(|lim| lim.min(*free))
                        .unwrap_or(*free)
                } else {
                    *free
                };
                let cap = if let Some(offloaded) = offloaded.as_ref() {
                    let mut spent = 0u64;
                    let mut taken = 0u32;
                    while (cursor + taken as usize) < offloaded.len() {
                        let next = spent.saturating_add(offloaded[cursor + taken as usize]);
                        if next > budget {
                            break;
                        }
                        spent = next;
                        taken += 1;
                    }
                    cursor += taken as usize;
                    taken
                } else if effective_per_layer == 0 {
                    remaining
                } else {
                    u32::try_from(budget / effective_per_layer).unwrap_or(remaining)
                };
                let assigned = cap.min(remaining);
                per_device[i] = assigned;
                remaining -= assigned;
            }
            if remaining > 0 {
                if let Some(last) = per_device.last_mut() {
                    *last += remaining;
                }
            }
            let total: u32 = per_device.iter().copied().sum::<u32>().min(total_layers);
            let weights: Vec<f32> = per_device.iter().map(|c| *c as f32).collect();
            MultiGpuDistribution {
                n_gpu_layers: total,
                tensor_split: if total > 0 {
                    normalize_weights(&weights)
                } else {
                    Vec::new()
                },
                main_gpu: Some(0),
                per_device_layers: per_device,
            }
        }
        "proportional" => {
            let effective_per_layer = bytes_per_layer.saturating_add(kv_bytes_per_layer);
            let weights: Vec<f32> = device_free_vram.iter().map(|f| *f as f32).collect();
            let split = normalize_weights(&weights);
            let capped_total = if let Some(costs) = exact {
                largest_total_that_fits(costs, auto_total, &split, device_free_vram)
            } else if effective_per_layer == 0 {
                auto_total
            } else {
                let feasible: u64 = device_free_vram
                    .iter()
                    .map(|free| free / effective_per_layer)
                    .sum();
                auto_total.min(u32::try_from(feasible).unwrap_or(auto_total))
            };
            MultiGpuDistribution {
                n_gpu_layers: capped_total,
                per_device_layers: distribute_by_weights(capped_total, &split),
                tensor_split: if capped_total > 0 { split } else { Vec::new() },
                main_gpu: None,
            }
        }
        // "balanced" and any unknown strategy fall through to an even split.
        _ => {
            let effective_per_layer = bytes_per_layer.saturating_add(kv_bytes_per_layer);
            let even: Vec<f32> = vec![1.0; n];
            let even_split = normalize_weights(&even);
            let exact_total = exact.map(|costs| {
                largest_total_that_fits(costs, auto_total, &even_split, device_free_vram)
            });
            let capacities: Vec<u32> = device_free_vram
                .iter()
                .enumerate()
                .map(|(index, free)| {
                    if let Some(total) = exact_total {
                        let base = total / n as u32;
                        let extra = u32::from((index as u32) < total % n as u32);
                        return base + extra;
                    }
                    if effective_per_layer == 0 {
                        auto_total
                    } else {
                        u32::try_from(free / effective_per_layer).unwrap_or(auto_total)
                    }
                })
                .collect();
            let mut per_device = vec![0u32; n];
            let mut remaining = auto_total;
            while remaining > 0 {
                let mut progressed = false;
                for (assigned, capacity) in per_device.iter_mut().zip(capacities.iter()) {
                    if remaining == 0 {
                        break;
                    }
                    if *assigned < *capacity {
                        *assigned += 1;
                        remaining -= 1;
                        progressed = true;
                    }
                }
                if !progressed {
                    break;
                }
            }
            let assigned_total = per_device.iter().copied().sum();
            let split: Vec<f32> = per_device.iter().map(|layers| *layers as f32).collect();
            MultiGpuDistribution {
                n_gpu_layers: assigned_total,
                per_device_layers: per_device,
                tensor_split: if assigned_total > 0 {
                    normalize_weights(&split)
                } else {
                    Vec::new()
                },
                main_gpu: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        candidate_gpu_layers, estimated_runtime_reserve_bytes, model_weight_split_bytes,
        plan_multi_gpu_distribution, reserve_device_vram, select_mtp_gpu_device,
        LlamaModelMetadata,
    };

    fn large_context_metadata() -> LlamaModelMetadata {
        LlamaModelMetadata {
            model_size_bytes: 16 * 1024 * 1024 * 1024,
            layer_count: 60,
            nextn_layer_count: 0,
            max_context_length: 262_144,
            n_embd: 4096,
            n_head: 32,
            n_head_kv: 8,
            n_embd_head_k: 128,
            n_embd_head_v: 128,
        }
    }

    #[test]
    fn mtp_uses_the_selected_device_with_the_most_free_vram() {
        let selected = [4, 7, 9];
        let free = [8, 24, 16];

        assert_eq!(select_mtp_gpu_device(&selected, &free), Some(7));
        assert_eq!(
            reserve_device_vram(&selected, &free, Some(7), 6),
            vec![8, 18, 16]
        );
    }

    #[test]
    fn runtime_reserve_holds_attention_scratch_when_flash_attention_disabled() {
        let available = 16_u64 * 1024 * 1024 * 1024;

        let reserve = estimated_runtime_reserve_bytes(
            &large_context_metadata(),
            available,
            32_768,
            2048,
            llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_DISABLED,
        );

        assert_eq!(reserve, available / 20 + 17_179_869_184);
    }

    #[test]
    fn runtime_reserve_assumes_flash_attention_for_auto_policy_on_every_backend() {
        let available = 16_u64 * 1024 * 1024 * 1024;

        let auto_reserve = estimated_runtime_reserve_bytes(
            &large_context_metadata(),
            available,
            32_768,
            2048,
            llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_AUTO,
        );
        let enabled_reserve = estimated_runtime_reserve_bytes(
            &large_context_metadata(),
            available,
            32_768,
            2048,
            llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_ENABLED,
        );

        assert_eq!(auto_reserve, enabled_reserve);
        assert_eq!(auto_reserve, available / 20);
    }

    #[test]
    fn metadata_counts_output_tensor_as_an_offload_layer() {
        let metadata = large_context_metadata();

        assert_eq!(metadata.offload_layer_count(), 61);
        assert_eq!(metadata.normalize_requested_gpu_layers(59), 59);
        assert_eq!(metadata.normalize_requested_gpu_layers(60), 61);
        assert_eq!(metadata.normalize_requested_gpu_layers(99), 61);
    }

    #[test]
    fn metadata_counts_bundled_nextn_and_output_layers() {
        let metadata = LlamaModelMetadata {
            nextn_layer_count: 1,
            ..large_context_metadata()
        };

        assert_eq!(metadata.model_layer_count(), 61);
        assert_eq!(metadata.offload_layer_count(), 62);
        assert_eq!(metadata.normalize_requested_gpu_layers(60), 62);
        assert_eq!(metadata.normalize_requested_gpu_layers(62), 62);
    }

    #[test]
    fn candidate_ladder_does_not_exceed_the_vram_estimate() {
        let candidates = candidate_gpu_layers(61, 60);

        assert_eq!(candidates.first(), Some(&60));
        assert!(!candidates.contains(&61));
        assert_eq!(candidates.last(), Some(&0));
    }

    #[test]
    fn full_offload_places_all_model_weights_on_gpu() {
        let metadata = large_context_metadata();

        let (cpu_bytes, gpu_bytes) =
            model_weight_split_bytes(&metadata, None, metadata.offload_layer_count());

        assert_eq!(cpu_bytes, 0);
        assert_eq!(gpu_bytes, metadata.model_size_bytes);
    }

    #[test]
    fn proportional_distribution_caps_total_to_per_device_free_capacity() {
        let dist =
            plan_multi_gpu_distribution("proportional", &[8, 24], 60, 1, 0, 60, None, None, None);

        assert_eq!(dist.n_gpu_layers, 32);
        assert_eq!(dist.per_device_layers, vec![8, 24]);
    }

    #[test]
    fn balanced_distribution_keeps_even_split_for_identical_cards() {
        let dist =
            plan_multi_gpu_distribution("balanced", &[16, 16], 60, 1, 0, 32, None, None, None);

        assert_eq!(dist.n_gpu_layers, 32);
        assert_eq!(dist.per_device_layers, vec![16, 16]);
        assert_eq!(dist.tensor_split, vec![0.5, 0.5]);
    }

    #[test]
    fn balanced_distribution_respects_a_sidecar_reduced_device_budget() {
        let dist =
            plan_multi_gpu_distribution("balanced", &[4, 16], 20, 1, 0, 16, None, None, None);

        assert_eq!(dist.n_gpu_layers, 16);
        assert_eq!(dist.per_device_layers, vec![4, 12]);
        assert_eq!(dist.tensor_split, vec![0.25, 0.75]);
    }
}

#[cfg(test)]
mod offload_cost_tests {
    use super::*;

    fn costs(units: &[u64]) -> ModelOffloadCosts {
        ModelOffloadCosts {
            unit_bytes: units.to_vec(),
        }
    }

    #[test]
    fn gpu_bytes_takes_the_last_units_output_layer_first() {
        let costs = costs(&[10, 20, 30, 1000]);
        assert_eq!(costs.gpu_bytes(0), 0);
        assert_eq!(
            costs.gpu_bytes(1),
            1000,
            "first unit offloaded is the output"
        );
        assert_eq!(costs.gpu_bytes(2), 1030);
        assert_eq!(costs.gpu_bytes(4), 1060);
        assert_eq!(costs.gpu_bytes(99), 1060, "saturates at the unit count");
    }

    #[test]
    fn a_heavy_output_layer_is_not_averaged_away() {
        let costs = costs(&[10, 20, 30, 1000]);
        assert_eq!(costs.max_units_within(900, &[]), 0);
        assert_eq!(costs.max_units_within(1000, &[]), 1);
        assert_eq!(costs.max_units_within(1029, &[]), 1);
        assert_eq!(costs.max_units_within(1030, &[]), 2);
    }

    #[test]
    fn kv_is_charged_per_block_but_not_for_the_output_unit() {
        let costs = costs(&[10, 20, 30, 1000]);
        assert_eq!(costs.max_units_within(1000, &[5, 5, 5]), 1);
        assert_eq!(costs.max_units_within(1034, &[5, 5, 5]), 1);
        assert_eq!(costs.max_units_within(1035, &[5, 5, 5]), 2);
    }

    #[test]
    fn kv_charges_stop_at_the_attention_layer_count() {
        let costs = costs(&[10, 20, 30, 1000]);
        assert_eq!(costs.max_units_within(1030, &[]), 2, "no KV charged at all");
    }

    fn qwen36_27b() -> LlamaModelMetadata {
        LlamaModelMetadata {
            model_size_bytes: 21_182_275_040,
            layer_count: 64,
            nextn_layer_count: 1,
            max_context_length: 262_144,
            n_embd: 5120,
            n_head: 24,
            n_head_kv: 4,
            n_embd_head_k: 256,
            n_embd_head_v: 256,
        }
    }

    #[test]
    fn kv_per_token_uses_declared_head_dims_not_n_embd_over_n_head() {
        assert_eq!(
            estimate_kv_bytes_per_token(&qwen36_27b(), Some("q8_0")),
            Some(139_264)
        );
        assert_eq!(
            estimate_kv_bytes_per_token(&qwen36_27b(), Some("f16")),
            Some(262_144)
        );
    }

    #[test]
    fn quantized_kv_types_include_their_block_scales() {
        assert_eq!(kv_bytes_per_value(Some("q8_0")), 34.0 / 32.0);
        assert_eq!(kv_bytes_per_value(Some("q4_0")), 18.0 / 32.0);
        assert_eq!(kv_bytes_per_value(Some("q6_k")), 210.0 / 256.0);
        assert_eq!(kv_bytes_per_value(Some("f16")), 2.0);
        assert_eq!(kv_bytes_per_value(None), 2.0, "llama.cpp defaults to f16");
    }

    #[test]
    fn head_dims_fall_back_to_the_division_when_undeclared() {
        let mut metadata = qwen36_27b();
        metadata.n_embd_head_k = 213;
        metadata.n_embd_head_v = 213;
        assert_eq!(
            estimate_kv_bytes_per_token(&metadata, Some("f16")),
            Some(218_112)
        );
    }

    fn gemma4_12b_geometry() -> KvCacheGeometry {
        let mut layers = Vec::new();
        for il in 0..48 {
            let global = il % 6 == 5;
            layers.push(llama_cpp_2::model::KvLayerGeometry {
                n_head_kv: if global { 1 } else { 8 },
                n_embd_head_k: 512,
                n_embd_head_v: 512,
                is_swa: !global,
            });
        }
        KvCacheGeometry {
            layers,
            n_swa: 1024,
        }
    }

    #[test]
    fn sliding_window_layers_are_capped_to_the_window() {
        let geometry = gemma4_12b_geometry();
        assert_eq!(geometry.cells_for_layer(false, 8192, 512), 8192);
        assert_eq!(geometry.cells_for_layer(true, 8192, 512), 1536);
        assert_eq!(geometry.cells_for_layer(true, 1024, 512), 1024);
    }

    #[test]
    fn gemma_kv_total_matches_the_iswa_cache_sizing() {
        let geometry = gemma4_12b_geometry();
        let total = geometry.total_bytes(8192, 512, Some("f16"));
        assert_eq!(total, 1_140_850_688);
    }

    #[test]
    fn recommended_context_is_solved_against_the_real_curve() {
        let geometry = gemma4_12b_geometry();
        let budget = geometry.total_bytes(8192, 512, Some("f16"));
        let solved = geometry.max_context_within(budget, 512, Some("f16"), 131_072);
        assert!(
            solved >= 8192,
            "solved {solved} should reach the probed context"
        );
        assert!(geometry.total_bytes(solved, 512, Some("f16")) <= budget);
        assert!(geometry.total_bytes(solved + 1, 512, Some("f16")) > budget);
    }

    #[test]
    fn per_layer_kv_bills_global_and_sliding_layers_differently() {
        let geometry = gemma4_12b_geometry();
        let per_layer = geometry.bytes_per_layer(8192, 512, Some("f16"));
        assert_eq!(per_layer[0], 1536 * 8 * 1024 * 2);
        assert_eq!(per_layer[5], 8192 * 1 * 1024 * 2);
    }

    #[test]
    fn multi_gpu_split_prices_the_output_layer_on_its_actual_device() {
        let units = vec![100u64, 100, 100, 1000];
        let uniform = plan_multi_gpu_distribution(
            "proportional",
            &[700, 700],
            4,
            325,
            0,
            4,
            None,
            None,
            None,
        );
        assert_eq!(
            uniform.n_gpu_layers, 4,
            "the flat average claims all four units fit"
        );

        let exact = plan_multi_gpu_distribution(
            "proportional",
            &[700, 700],
            4,
            325,
            0,
            4,
            None,
            None,
            Some(&units),
        );
        assert!(
            exact.n_gpu_layers < 4,
            "the 1000-byte output unit cannot fit on a 700-byte device"
        );
    }

    #[test]
    fn block_index_parses_only_repeating_layers() {
        assert_eq!(block_index("blk.0.attn_q.weight"), Some(0));
        assert_eq!(block_index("blk.64.nextn.eh_proj.weight"), Some(64));
        assert_eq!(block_index("output.weight"), None);
        assert_eq!(block_index("token_embd.weight"), None);
        assert_eq!(block_index("blk.notanumber.weight"), None);
    }
}

#[cfg(test)]
mod real_model_plan {
    use super::*;

    #[test]
    #[ignore]
    fn print_plan_for_real_model() {
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let free_vram: u64 = std::env::var("LETTUCE_PLAN_FREE_VRAM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(6_759_383_040);
        let ctx: u32 = std::env::var("LETTUCE_PLAN_CTX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(16_384);
        let bundled: bool = std::env::var("LETTUCE_PLAN_MTP").is_ok();

        let costs = load_offload_costs(&path).expect("unit costs");
        let metadata = load_model_metadata(&path).expect("metadata");
        let geometry = load_kv_geometry(&path);

        println!(
            "units={} layer_count={}",
            costs.unit_count(),
            metadata.layer_count
        );
        println!(
            "head_k={} head_v={}",
            metadata.n_embd_head_k, metadata.n_embd_head_v
        );
        match geometry.as_ref() {
            Some(g) => println!(
                "kv layers={} n_swa={} swa_layers={} kv_total@ctx={}",
                g.layers.len(),
                g.n_swa,
                g.layers.iter().filter(|l| l.is_swa).count(),
                g.total_bytes(ctx, 512, Some("q8_0"))
            ),
            None => println!("kv geometry unavailable"),
        }
        println!("output unit bytes={}", costs.gpu_bytes(1));
        let guessed = estimated_runtime_reserve_bytes(
            &metadata,
            free_vram,
            ctx,
            512,
            llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_AUTO,
        );
        let measured = measure_device_compute_bytes(
            &path,
            9,
            ctx,
            512,
            Some(true),
            Some("q8_0"),
            llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_AUTO,
        );
        println!("compute reserve guessed={guessed} measured={measured:?}");

        let plan = plan_smart_gpu_offload(
            &path,
            Some(32 * 1024 * 1024 * 1024),
            Some(free_vram),
            Some(ctx),
            512,
            Some(true),
            Some("q8_0"),
            llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_AUTO,
            0,
            bundled,
        )
        .expect("plan");
        println!(
            "PLAN layers={} of {} ctx={} kv={} runtime_reserve={} budget={}",
            plan.estimated_gpu_layers,
            plan.total_layers,
            plan.planned_context,
            plan.estimated_kv_bytes,
            plan.estimated_runtime_reserve_bytes,
            plan.effective_vram_budget_bytes
        );
        println!(
            "weights for that plan = {}",
            costs.gpu_bytes(plan.estimated_gpu_layers)
        );
    }
}
