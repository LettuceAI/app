//! GPU offload, KV cache and context planning for llama.cpp models.
//!
//! The formulas are the legacy desktop planner's, carried over unchanged:
//! offload units are the repeating blocks plus the output layer (offloaded
//! first), per-unit weights come from the GGUF tensor index, KV cache bytes
//! come from per-layer geometry with sliding-window layers capped to the
//! window, and the recommended context is solved against the real KV curve.
//! Loading a model or measuring a compute buffer happens in the runtime
//! adapter, which hands the results in.

use std::collections::BTreeMap;

/// How llama.cpp decides on flash attention for a context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FlashAttentionPolicy {
    Auto,
    Disabled,
    Enabled,
}

/// One attention layer's KV shape, read from the loaded model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KvLayerGeometry {
    pub n_head_kv: u32,
    pub n_embd_head_k: u32,
    pub n_embd_head_v: u32,
    pub is_swa: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LlamaModelMetadata {
    pub model_size_bytes: u64,
    pub layer_count: u32,
    pub nextn_layer_count: u32,
    pub max_context_length: u32,
    pub n_embd: u64,
    pub n_head: u64,
    pub n_head_kv: u64,
    pub n_embd_head_k: u64,
    pub n_embd_head_v: u64,
}

impl LlamaModelMetadata {
    pub fn model_layer_count(&self) -> u32 {
        self.layer_count
            .max(1)
            .saturating_add(self.nextn_layer_count)
    }

    pub fn offload_layer_count(&self) -> u32 {
        self.model_layer_count().saturating_add(1)
    }

    pub fn normalize_requested_gpu_layers(&self, requested: u32) -> u32 {
        if requested >= self.layer_count.max(1) {
            self.offload_layer_count()
        } else {
            requested
        }
    }
}

#[derive(Clone, Debug)]
pub struct SmartGpuOffloadPlan {
    pub total_layers: u32,
    pub recommended_context: Option<u32>,
    pub planned_context: u32,
    pub estimated_gpu_layers: u32,
    pub candidate_gpu_layers: Vec<u32>,
    pub kqv_vram_reserved: bool,
    pub planning_offload_kqv: Option<bool>,
    pub estimated_kv_bytes: u64,
    pub kv_bytes_per_layer: u64,
    pub estimated_sidecar_vram_reserve_bytes: u64,
    pub estimated_runtime_reserve_bytes: u64,
    pub effective_vram_budget_bytes: u64,
    pub bytes_per_layer: u64,
    pub offload_unit_costs: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelOffloadCosts {
    unit_bytes: Vec<u64>,
}

impl ModelOffloadCosts {
    /// Per-unit weight bytes from a GGUF tensor index: `blk.N.*` tensors
    /// belong to unit N, `token_embd*` stays on the CPU, and everything else
    /// is the output unit, which also carries the embedding when the model
    /// ties its head (no `output.weight`).
    pub fn from_tensors<'a>(tensors: impl IntoIterator<Item = (&'a str, u64)>) -> Option<Self> {
        let mut blocks: BTreeMap<u32, u64> = BTreeMap::new();
        let mut output_bytes = 0u64;
        let mut input_bytes = 0u64;
        let mut has_output_weight = false;

        for (name, size) in tensors {
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

        Some(Self { unit_bytes })
    }

    pub fn unit_count(&self) -> u32 {
        u32::try_from(self.unit_bytes.len()).unwrap_or(u32::MAX)
    }

    pub fn gpu_bytes(&self, gpu_layers: u32) -> u64 {
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

/// The K and V cache types. Legacy set one type for both; with one shared
/// type every formula is the legacy one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KvCacheTypes<'a> {
    pub k: Option<&'a str>,
    pub v: Option<&'a str>,
}

impl<'a> KvCacheTypes<'a> {
    #[must_use]
    pub const fn uniform(kv_type: Option<&'a str>) -> Self {
        Self {
            k: kv_type,
            v: kv_type,
        }
    }

    /// Separate K/V types when either is set, else the shared type.
    #[must_use]
    pub const fn from_settings(
        kv_type: Option<&'a str>,
        k: Option<&'a str>,
        v: Option<&'a str>,
    ) -> Self {
        if k.is_some() || v.is_some() {
            Self { k, v }
        } else {
            Self::uniform(kv_type)
        }
    }

    /// The one type both halves use, if they agree.
    #[must_use]
    pub fn shared(&self) -> Option<Option<&'a str>> {
        let normalize = |value: Option<&str>| value.map(|value| value.trim().to_ascii_lowercase());
        (normalize(self.k) == normalize(self.v)).then_some(self.k)
    }

    /// The shared type, or `k=<type>,v=<type>` when they differ.
    #[must_use]
    pub fn label(&self) -> Option<String> {
        match self.shared() {
            Some(shared) => shared.map(ToOwned::to_owned),
            None => Some(format!(
                "k={},v={}",
                self.k.unwrap_or("f16"),
                self.v.unwrap_or("f16")
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvCacheGeometry {
    layers: Vec<KvLayerGeometry>,
    n_swa: u32,
}

fn block_index(tensor_name: &str) -> Option<u32> {
    let rest = tensor_name.strip_prefix("blk.")?;
    let (digits, _) = rest.split_once('.')?;
    digits.parse().ok()
}

impl KvCacheGeometry {
    /// `None` when the model reports no attention layers.
    #[must_use]
    pub fn new(layers: Vec<KvLayerGeometry>, n_swa: u32) -> Option<Self> {
        (!layers.is_empty()).then_some(Self { layers, n_swa })
    }

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
        kv_types: KvCacheTypes<'_>,
    ) -> Vec<u64> {
        let shared = kv_types.shared();
        let k_bytes_per_value = kv_bytes_per_value(kv_types.k);
        let v_bytes_per_value = kv_bytes_per_value(kv_types.v);
        self.layers
            .iter()
            .map(|layer| {
                let cells = self.cells_for_layer(layer.is_swa, planned_context, n_ubatch);
                let n_head_kv = u64::from(layer.n_head_kv);
                if let Some(shared) = shared {
                    let per_cell = n_head_kv.saturating_mul(
                        u64::from(layer.n_embd_head_k) + u64::from(layer.n_embd_head_v),
                    );
                    return ((cells.saturating_mul(per_cell)) as f64 * kv_bytes_per_value(shared))
                        as u64;
                }
                let k_values =
                    cells.saturating_mul(n_head_kv.saturating_mul(u64::from(layer.n_embd_head_k)));
                let v_values =
                    cells.saturating_mul(n_head_kv.saturating_mul(u64::from(layer.n_embd_head_v)));
                (k_values as f64 * k_bytes_per_value + v_values as f64 * v_bytes_per_value) as u64
            })
            .collect()
    }

    pub fn total_bytes(
        &self,
        planned_context: u32,
        n_ubatch: u32,
        kv_types: KvCacheTypes<'_>,
    ) -> u64 {
        self.bytes_per_layer(planned_context, n_ubatch, kv_types)
            .into_iter()
            .fold(0u64, |acc, bytes| acc.saturating_add(bytes))
    }

    fn max_context_within(
        &self,
        budget: u64,
        n_ubatch: u32,
        kv_types: KvCacheTypes<'_>,
        max_context: u32,
    ) -> u32 {
        if self.total_bytes(1, n_ubatch, kv_types) > budget {
            return 0;
        }
        let (mut lo, mut hi) = (1u32, max_context.max(1));
        if self.total_bytes(hi, n_ubatch, kv_types) <= budget {
            return hi;
        }
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            if self.total_bytes(mid, n_ubatch, kv_types) <= budget {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }
}

fn kv_bytes_per_value(kv_type: Option<&str>) -> f64 {
    match kv_type
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
    kv_types: KvCacheTypes<'_>,
) -> Option<u64> {
    let n_layer = u64::from(metadata.layer_count.max(1));
    let n_head_kv = metadata.n_head_kv.max(1);
    let bytes = match kv_types.shared() {
        Some(shared) => {
            let head_bytes = metadata.n_embd_head_k.max(1) + metadata.n_embd_head_v.max(1);
            (n_layer as f64) * (n_head_kv as f64) * (head_bytes as f64) * kv_bytes_per_value(shared)
        }
        None => {
            let head_k = metadata.n_embd_head_k.max(1) as f64 * kv_bytes_per_value(kv_types.k);
            let head_v = metadata.n_embd_head_v.max(1) as f64 * kv_bytes_per_value(kv_types.v);
            (n_layer as f64) * (n_head_kv as f64) * (head_k + head_v)
        }
    };
    Some(bytes.max(0.0) as u64)
}

fn default_memory_reserve_bytes(available_memory_bytes: u64) -> u64 {
    (available_memory_bytes / 5).max(512 * 1024 * 1024)
}

fn ram_budget_for_context(metadata: &LlamaModelMetadata, available_memory_bytes: u64) -> u64 {
    let reserve = default_memory_reserve_bytes(available_memory_bytes);
    available_memory_bytes.saturating_sub(metadata.model_size_bytes.saturating_add(reserve))
}

#[expect(
    clippy::too_many_arguments,
    reason = "the legacy planner's inputs, kept as they were"
)]
fn compute_recommended_context(
    metadata: &LlamaModelMetadata,
    geometry: Option<&KvCacheGeometry>,
    n_ubatch: u32,
    gpu_weight_bytes: u64,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    llama_offload_kqv: Option<bool>,
    kv_types: KvCacheTypes<'_>,
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
            kv_types,
            metadata.max_context_length,
        ));
    }
    let kv_bytes_per_token = estimate_kv_bytes_per_token(metadata, kv_types)?;
    if kv_bytes_per_token == 0 {
        return None;
    }
    let mut recommended = available_for_ctx / kv_bytes_per_token;
    if recommended > u64::from(metadata.max_context_length) {
        recommended = u64::from(metadata.max_context_length);
    }
    Some(recommended as u32)
}

fn push_unique(out: &mut Vec<u32>, value: u32) {
    if !out.contains(&value) {
        out.push(value);
    }
}

const ATTENTION_SCORE_BYTES: u64 = 4;
const COMPUTE_BUFFER_SAFETY_FACTOR: u64 = 2;
const COMPUTE_RESERVE_FLOOR_BYTES: u64 = 256 * 1024 * 1024;

/// VRAM a bundled MTP draft needs beside the target: its weights plus a
/// second KV cache of the planned context.
#[must_use]
pub fn estimate_mtp_gpu_reserve_bytes(
    metadata: &LlamaModelMetadata,
    geometry: Option<&KvCacheGeometry>,
    planned_context: u32,
    n_ubatch: u32,
    kv_types: KvCacheTypes<'_>,
) -> u64 {
    let draft_kv_bytes = match geometry {
        Some(geometry) => geometry.total_bytes(planned_context, n_ubatch, kv_types),
        None => estimate_kv_bytes_per_token(metadata, kv_types)
            .unwrap_or(0)
            .saturating_mul(u64::from(planned_context.max(1))),
    };
    metadata.model_size_bytes.saturating_add(draft_kv_bytes)
}

pub fn select_mtp_gpu_device(
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

pub fn reserve_device_vram(
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
    flash_attention_policy: FlashAttentionPolicy,
) -> u64 {
    let floor = (available_vram_bytes / 20).max(COMPUTE_RESERVE_FLOOR_BYTES);
    let attention_reserve = if flash_attention_policy != FlashAttentionPolicy::Disabled {
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

pub fn context_bucket_upper(context: u32) -> u32 {
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

pub fn merge_cached_candidate_layers(
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

pub fn model_weight_split_bytes(
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

#[expect(
    clippy::too_many_arguments,
    reason = "the legacy planner's inputs, kept as they were"
)]
pub fn compute_recommended_context_for_gpu_layers(
    metadata: &LlamaModelMetadata,
    costs: Option<&ModelOffloadCosts>,
    geometry: Option<&KvCacheGeometry>,
    n_ubatch: u32,
    available_memory_bytes: Option<u64>,
    available_vram_bytes: Option<u64>,
    gpu_layers: u32,
    llama_offload_kqv: Option<bool>,
    kv_types: KvCacheTypes<'_>,
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
            kv_types,
            metadata.max_context_length,
        ));
    }
    let kv_bytes_per_token = estimate_kv_bytes_per_token(metadata, kv_types)?;
    if kv_bytes_per_token == 0 {
        return None;
    }
    let mut recommended = available_for_ctx / kv_bytes_per_token;
    if recommended > u64::from(metadata.max_context_length) {
        recommended = u64::from(metadata.max_context_length);
    }
    Some(recommended as u32)
}

/// What the planner knows about one model file.
#[derive(Clone, Copy, Debug)]
pub struct OffloadModel<'a> {
    pub metadata: LlamaModelMetadata,
    pub costs: Option<&'a ModelOffloadCosts>,
    pub geometry: Option<&'a KvCacheGeometry>,
}

/// The requested runtime shape the plan is made for.
#[derive(Clone, Copy, Debug)]
pub struct OffloadRequest<'a> {
    pub available_memory_bytes: Option<u64>,
    pub available_vram_bytes: Option<u64>,
    pub requested_context: Option<u32>,
    pub n_batch: u32,
    pub resolved_offload_kqv: Option<bool>,
    pub kv_types: KvCacheTypes<'a>,
    pub flash_attention_policy: FlashAttentionPolicy,
    pub sidecar_vram_reserve_bytes: u64,
    pub bundled_mtp_draft: bool,
}

/// Plans GPU layers and context for one model. `measure_compute` is asked
/// once, for `(gpu_layers, planned_context)`, for the device compute buffer
/// llama.cpp would allocate; `None` keeps the estimated reserve.
pub fn plan_smart_gpu_offload(
    model: OffloadModel<'_>,
    request: OffloadRequest<'_>,
    measure_compute: impl FnOnce(u32, u32) -> Option<u64>,
) -> SmartGpuOffloadPlan {
    let OffloadRequest {
        available_memory_bytes,
        available_vram_bytes,
        requested_context,
        n_batch,
        resolved_offload_kqv,
        kv_types,
        flash_attention_policy,
        sidecar_vram_reserve_bytes,
        bundled_mtp_draft,
    } = request;
    let metadata = model.metadata;
    let costs = model.costs;
    let total_layers = costs
        .map(ModelOffloadCosts::unit_count)
        .unwrap_or_else(|| metadata.offload_layer_count());
    let geometry = model.geometry;
    let available_vram = available_vram_bytes.unwrap_or(0);
    let effective_vram_budget_bytes = available_vram.saturating_mul(9) / 10;
    let bytes_per_layer = metadata
        .model_size_bytes
        .checked_add(u64::from(metadata.model_layer_count()) - 1)
        .and_then(|bytes| bytes.checked_div(u64::from(metadata.model_layer_count())))
        .unwrap_or(0);
    let kv_bytes_per_token = estimate_kv_bytes_per_token(&metadata, kv_types).unwrap_or(0);
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
        let per_block = if let Some(geometry) = geometry {
            geometry
                .bytes_per_layer(planned_context, n_batch, kv_types)
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
            let layers = match costs {
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
        geometry,
        n_batch,
        0,
        available_memory_bytes,
        available_vram_bytes,
        resolved_offload_kqv,
        kv_types,
    );
    let mut planned_context = requested_context
        .or(recommended_context)
        .unwrap_or(metadata.max_context_length)
        .clamp(1, metadata.max_context_length);
    let (mut kv_per_block, mut kv_bytes_per_layer) = kv_for_context(planned_context);
    let (mut estimated_gpu_layers, mut estimated_runtime_reserve_bytes) =
        layers_for_context(planned_context, &kv_per_block, None);

    let measured_compute = measure_compute(estimated_gpu_layers.max(1), planned_context);
    if measured_compute.is_some() {
        let refreshed = layers_for_context(planned_context, &kv_per_block, measured_compute);
        estimated_gpu_layers = refreshed.0;
        estimated_runtime_reserve_bytes = refreshed.1;
    }

    if requested_context.is_none() {
        let resident_weights = costs
            .map(|costs| costs.gpu_bytes(estimated_gpu_layers))
            .unwrap_or_else(|| model_weight_split_bytes(&metadata, None, estimated_gpu_layers).1);
        recommended_context = compute_recommended_context(
            &metadata,
            geometry,
            n_batch,
            resident_weights,
            available_memory_bytes,
            available_vram_bytes,
            resolved_offload_kqv,
            kv_types,
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
        .map(|costs| costs.combined_units(&kv_per_block))
        .unwrap_or_default();

    SmartGpuOffloadPlan {
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
    }
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
pub struct MultiGpuDistribution {
    pub n_gpu_layers: u32,
    pub tensor_split: Vec<f32>,
    pub main_gpu: Option<i32>,
    pub per_device_layers: Vec<u32>,
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
#[expect(
    clippy::too_many_arguments,
    reason = "the legacy planner's inputs, kept as they were"
)]
pub fn plan_multi_gpu_distribution(
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
                } else {
                    budget
                        .checked_div(effective_per_layer)
                        .map_or(remaining, |cap| u32::try_from(cap).unwrap_or(remaining))
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
                    free.checked_div(effective_per_layer)
                        .map_or(auto_total, |cap| u32::try_from(cap).unwrap_or(auto_total))
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
        FlashAttentionPolicy, LlamaModelMetadata, candidate_gpu_layers,
        estimated_runtime_reserve_bytes, model_weight_split_bytes, plan_multi_gpu_distribution,
        reserve_device_vram, select_mtp_gpu_device,
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
            FlashAttentionPolicy::Disabled,
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
            FlashAttentionPolicy::Auto,
        );
        let enabled_reserve = estimated_runtime_reserve_bytes(
            &large_context_metadata(),
            available,
            32_768,
            2048,
            FlashAttentionPolicy::Enabled,
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
            estimate_kv_bytes_per_token(&qwen36_27b(), KvCacheTypes::uniform(Some("q8_0"))),
            Some(139_264)
        );
        assert_eq!(
            estimate_kv_bytes_per_token(&qwen36_27b(), KvCacheTypes::uniform(Some("f16"))),
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
            estimate_kv_bytes_per_token(&metadata, KvCacheTypes::uniform(Some("f16"))),
            Some(218_112)
        );
    }

    fn gemma4_12b_geometry() -> KvCacheGeometry {
        let mut layers = Vec::new();
        for il in 0..48 {
            let global = il % 6 == 5;
            layers.push(KvLayerGeometry {
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
        let total = geometry.total_bytes(8192, 512, KvCacheTypes::uniform(Some("f16")));
        assert_eq!(total, 1_140_850_688);
    }

    #[test]
    fn recommended_context_is_solved_against_the_real_curve() {
        let geometry = gemma4_12b_geometry();
        let budget = geometry.total_bytes(8192, 512, KvCacheTypes::uniform(Some("f16")));
        let solved =
            geometry.max_context_within(budget, 512, KvCacheTypes::uniform(Some("f16")), 131_072);
        assert!(
            solved >= 8192,
            "solved {solved} should reach the probed context"
        );
        assert!(geometry.total_bytes(solved, 512, KvCacheTypes::uniform(Some("f16"))) <= budget);
        assert!(geometry.total_bytes(solved + 1, 512, KvCacheTypes::uniform(Some("f16"))) > budget);
    }

    #[test]
    fn per_layer_kv_bills_global_and_sliding_layers_differently() {
        let geometry = gemma4_12b_geometry();
        let per_layer = geometry.bytes_per_layer(8192, 512, KvCacheTypes::uniform(Some("f16")));
        assert_eq!(per_layer[0], 1536 * 8 * 1024 * 2);
        assert_eq!(per_layer[5], 8192 * 1024 * 2);
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
    fn tensor_index_charges_blocks_output_and_a_tied_embedding() {
        let untied = ModelOffloadCosts::from_tensors([
            ("token_embd.weight", 500),
            ("blk.0.attn_q.weight", 10),
            ("blk.0.ffn_up.weight", 5),
            ("blk.1.attn_q.weight", 20),
            ("output_norm.weight", 1),
            ("output.weight", 900),
        ])
        .expect("costs");
        assert_eq!(untied.unit_bytes, vec![15, 20, 901]);
        let tied = ModelOffloadCosts::from_tensors([
            ("token_embd.weight", 500),
            ("blk.0.attn_q.weight", 10),
            ("output_norm.weight", 1),
        ])
        .expect("costs");
        assert_eq!(tied.unit_bytes, vec![10, 501]);
        assert_eq!(
            ModelOffloadCosts::from_tensors([("output.weight", 900)]),
            None
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

    #[test]
    fn equal_k_and_v_types_are_the_shared_type() {
        let mixed_case = KvCacheTypes {
            k: Some("Q8_0"),
            v: Some(" q8_0"),
        };
        assert_eq!(mixed_case.shared(), Some(Some("Q8_0")));
        assert_eq!(KvCacheTypes::uniform(None).label(), None);
        assert_eq!(
            KvCacheTypes::uniform(Some("q4_0")).label().as_deref(),
            Some("q4_0")
        );
        let split = KvCacheTypes::from_settings(Some("f32"), Some("q8_0"), None);
        assert_eq!(split.label().as_deref(), Some("k=q8_0,v=f16"));
        assert_eq!(
            KvCacheTypes::from_settings(Some("q8_0"), None, None),
            KvCacheTypes::uniform(Some("q8_0"))
        );
    }

    #[test]
    fn split_kv_types_bill_each_half_by_its_own_type() {
        let split = KvCacheTypes {
            k: Some("f16"),
            v: Some("q8_0"),
        };
        assert_eq!(
            estimate_kv_bytes_per_token(&qwen36_27b(), split),
            Some(262_144 / 2 + 139_264 / 2)
        );
        let geometry = gemma4_12b_geometry();
        let per_layer = geometry.bytes_per_layer(8192, 512, split);
        assert_eq!(per_layer[0], 1536 * 8 * 512 * 2 + 1536 * 8 * 512 * 34 / 32);
        assert_eq!(per_layer[5], 8192 * 512 * 2 + 8192 * 512 * 34 / 32);
        let same = KvCacheTypes {
            k: Some("q8_0"),
            v: Some("q8_0"),
        };
        assert_eq!(
            geometry.total_bytes(8192, 512, same),
            geometry.total_bytes(8192, 512, KvCacheTypes::uniform(Some("q8_0")))
        );
    }
}
