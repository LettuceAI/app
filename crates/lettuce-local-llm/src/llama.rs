//! llama.cpp bindings for the offload planner: the process-wide backend,
//! model metadata, KV geometry and per-unit weights read from a model file,
//! and compute buffers measured without allocating. Results are cached per
//! model path for the life of the process, as the legacy runtime did.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use llama_cpp_2::context::params::{KvCacheType, LlamaContextParams};
use llama_cpp_2::gguf::GgufContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_sys_2::llama_flash_attn_type;

use crate::offload::{
    FlashAttentionPolicy, KvCacheGeometry, KvLayerGeometry, LlamaModelMetadata, ModelOffloadCosts,
    OffloadModel, OffloadRequest, SmartGpuOffloadPlan,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LlamaRuntimeError {
    #[error("failed to initialize the llama.cpp backend: {0}")]
    Backend(String),
    #[error("failed to load llama model metadata for smart offload: {0}")]
    ModelLoad(String),
    #[error("llama.cpp metadata cache lock poisoned")]
    CachePoisoned,
}

static SHARED_BACKEND: OnceLock<Arc<LlamaBackend>> = OnceLock::new();

/// The process-wide llama.cpp backend; llama.cpp allows one.
pub fn shared_backend() -> Result<Arc<LlamaBackend>, LlamaRuntimeError> {
    if let Some(backend) = SHARED_BACKEND.get() {
        return Ok(backend.clone());
    }
    let backend = Arc::new(
        LlamaBackend::init().map_err(|error| LlamaRuntimeError::Backend(error.to_string()))?,
    );
    let _ = SHARED_BACKEND.set(backend);
    SHARED_BACKEND
        .get()
        .cloned()
        .ok_or_else(|| LlamaRuntimeError::Backend("failed to cache the shared backend".into()))
}

#[must_use]
pub fn flash_attention_type(policy: FlashAttentionPolicy) -> llama_flash_attn_type {
    match policy {
        FlashAttentionPolicy::Auto => llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_AUTO,
        FlashAttentionPolicy::Disabled => llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_DISABLED,
        FlashAttentionPolicy::Enabled => llama_cpp_sys_2::LLAMA_FLASH_ATTN_TYPE_ENABLED,
    }
}

#[must_use]
pub fn parse_kv_cache_type(value: &str) -> Option<KvCacheType> {
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
    flash_attention_policy: FlashAttentionPolicy,
    kv_type_hash: u64,
}

type Cache<K, V> = OnceLock<Mutex<HashMap<K, V>>>;

static COMPUTE_PROBE_CACHE: Cache<ComputeProbeKey, Option<u64>> = OnceLock::new();
static KV_GEOMETRY_CACHE: Cache<String, Option<KvCacheGeometry>> = OnceLock::new();
static MODEL_OFFLOAD_COSTS_CACHE: Cache<String, Option<ModelOffloadCosts>> = OnceLock::new();
static MODEL_METADATA_CACHE: Cache<String, LlamaModelMetadata> = OnceLock::new();

fn cache<K, V>(cell: &'static Cache<K, V>) -> &'static Mutex<HashMap<K, V>> {
    cell.get_or_init(|| Mutex::new(HashMap::new()))
}

fn stable_hash(value: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// The device compute buffer llama.cpp would allocate for this shape,
/// projected without allocating memory.
pub fn measure_device_compute_bytes(
    model_path: &str,
    n_gpu_layers: u32,
    planned_context: u32,
    n_batch: u32,
    offload_kqv: Option<bool>,
    llama_kv_type: Option<&str>,
    flash_attention_policy: FlashAttentionPolicy,
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
    if let Some(cached) = cache(&COMPUTE_PROBE_CACHE).lock().ok()?.get(&key).copied() {
        return cached;
    }

    let mut model_params = LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);
    let mut context_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(planned_context.max(1)))
        .with_n_batch(n_batch.max(1))
        .with_n_ubatch(n_batch.max(1))
        .with_flash_attention_policy(flash_attention_type(flash_attention_policy));
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

    if let Ok(mut cache) = cache(&COMPUTE_PROBE_CACHE).lock() {
        cache.insert(key, measured);
    }
    measured
}

fn load_kv_geometry_from_model(model: &LlamaModel) -> Option<KvCacheGeometry> {
    let geometry = model.kv_geometry()?;
    KvCacheGeometry::new(
        geometry
            .layers
            .into_iter()
            .map(|layer| KvLayerGeometry {
                n_head_kv: layer.n_head_kv,
                n_embd_head_k: layer.n_embd_head_k,
                n_embd_head_v: layer.n_embd_head_v,
                is_swa: layer.is_swa,
            })
            .collect(),
        geometry.n_swa,
    )
}

/// The model's per-layer KV shape, read when its metadata is first loaded.
pub fn load_kv_geometry(model_path: &str) -> Option<KvCacheGeometry> {
    if let Some(cached) = cache(&KV_GEOMETRY_CACHE)
        .lock()
        .ok()?
        .get(model_path)
        .cloned()
    {
        return cached;
    }
    load_model_metadata(model_path).ok()?;
    cache(&KV_GEOMETRY_CACHE)
        .lock()
        .ok()?
        .get(model_path)
        .cloned()?
}

fn load_offload_costs_uncached(model_path: &str) -> Option<ModelOffloadCosts> {
    let gguf = GgufContext::from_file(Path::new(model_path))?;
    let tensors = (0..gguf.n_tensors())
        .filter_map(|index| Some((gguf.tensor_name(index)?, gguf.tensor_size(index))))
        .collect::<Vec<_>>();
    ModelOffloadCosts::from_tensors(tensors)
}

/// Per-unit weight bytes from the model's GGUF tensor index.
pub fn load_offload_costs(model_path: &str) -> Option<ModelOffloadCosts> {
    if let Some(costs) = cache(&MODEL_OFFLOAD_COSTS_CACHE)
        .lock()
        .ok()?
        .get(model_path)
        .cloned()
    {
        return costs;
    }
    let costs = load_offload_costs_uncached(model_path);
    cache(&MODEL_OFFLOAD_COSTS_CACHE)
        .lock()
        .ok()?
        .insert(model_path.to_string(), costs.clone());
    costs
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

fn load_model_metadata_uncached(model_path: &str) -> Result<LlamaModelMetadata, LlamaRuntimeError> {
    let backend = shared_backend()?;
    let model = LlamaModel::load_from_file(
        backend.as_ref(),
        model_path,
        &LlamaModelParams::default().with_n_gpu_layers(0),
    )
    .map_err(|error| LlamaRuntimeError::ModelLoad(error.to_string()))?;

    let n_embd = u64::try_from(model.n_embd()).unwrap_or(0).max(1);
    let n_head = u64::from(model.n_head()).max(1);
    let implied_head_dim = (n_embd / n_head).max(1);
    let (n_embd_head_k, n_embd_head_v) = gguf_head_dims(model_path, implied_head_dim);

    if let Ok(mut cache) = cache(&KV_GEOMETRY_CACHE).lock() {
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

/// The model's shape, loaded once per path (weights stay on the CPU).
pub fn load_model_metadata(model_path: &str) -> Result<LlamaModelMetadata, LlamaRuntimeError> {
    if let Some(metadata) = cache(&MODEL_METADATA_CACHE)
        .lock()
        .map_err(|_| LlamaRuntimeError::CachePoisoned)?
        .get(model_path)
        .copied()
    {
        return Ok(metadata);
    }
    let metadata = load_model_metadata_uncached(model_path)?;
    cache(&MODEL_METADATA_CACHE)
        .lock()
        .map_err(|_| LlamaRuntimeError::CachePoisoned)?
        .insert(model_path.to_string(), metadata);
    Ok(metadata)
}

/// Plans GPU offload for a model file, measuring its compute buffer.
pub fn plan_smart_gpu_offload(
    model_path: &str,
    request: OffloadRequest<'_>,
) -> Result<SmartGpuOffloadPlan, LlamaRuntimeError> {
    let metadata = load_model_metadata(model_path)?;
    let costs = load_offload_costs(model_path);
    let geometry = load_kv_geometry(model_path);
    Ok(crate::offload::plan_smart_gpu_offload(
        OffloadModel {
            metadata,
            costs: costs.as_ref(),
            geometry: geometry.as_ref(),
        },
        request,
        |n_gpu_layers, planned_context| {
            measure_device_compute_bytes(
                model_path,
                n_gpu_layers,
                planned_context,
                request.n_batch,
                request.resolved_offload_kqv,
                request.llama_kv_type,
                request.flash_attention_policy,
            )
        },
    ))
}

/// VRAM a bundled MTP draft of this model needs.
pub fn estimate_mtp_gpu_reserve_bytes(
    model_path: &str,
    planned_context: u32,
    n_ubatch: u32,
    llama_kv_type: Option<&str>,
) -> Result<u64, LlamaRuntimeError> {
    let metadata = load_model_metadata(model_path)?;
    Ok(crate::offload::estimate_mtp_gpu_reserve_bytes(
        &metadata,
        load_kv_geometry(model_path).as_ref(),
        planned_context,
        n_ubatch,
        llama_kv_type,
    ))
}

#[cfg(test)]
mod real_model_plan {
    use super::*;

    #[test]
    #[ignore = "needs a local GGUF model in LETTUCE_PLAN_MODEL"]
    fn print_plan_for_real_model() {
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let free_vram: u64 = std::env::var("LETTUCE_PLAN_FREE_VRAM")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(6_759_383_040);
        let ctx: u32 = std::env::var("LETTUCE_PLAN_CTX")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(16_384);
        let bundled = std::env::var("LETTUCE_PLAN_MTP").is_ok();

        let costs = load_offload_costs(&path).expect("unit costs");
        let metadata = load_model_metadata(&path).expect("metadata");
        let geometry = load_kv_geometry(&path);
        println!(
            "units={} layer_count={} head_k={} head_v={}",
            costs.unit_count(),
            metadata.layer_count,
            metadata.n_embd_head_k,
            metadata.n_embd_head_v
        );
        if let Some(geometry) = geometry.as_ref() {
            println!(
                "kv_total@ctx={}",
                geometry.total_bytes(ctx, 512, Some("q8_0"))
            );
        }
        println!("output unit bytes={}", costs.gpu_bytes(1));
        let plan = plan_smart_gpu_offload(
            &path,
            OffloadRequest {
                available_memory_bytes: Some(32 * 1024 * 1024 * 1024),
                available_vram_bytes: Some(free_vram),
                requested_context: Some(ctx),
                n_batch: 512,
                resolved_offload_kqv: Some(true),
                llama_kv_type: Some("q8_0"),
                flash_attention_policy: FlashAttentionPolicy::Auto,
                sidecar_vram_reserve_bytes: 0,
                bundled_mtp_draft: bundled,
            },
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
    }
}
