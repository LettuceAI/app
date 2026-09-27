//! The download planner, what the model browser shows while a model is set
//! up before it is downloaded: the longest context each KV cache type allows
//! next to the chosen sidecars, how many layers llama.cpp can offload, and
//! for one choice of file, KV type, context, offload and KV placement its
//! score, memory, GPU plan and a better quantization. Its bytes per KV value
//! and its score are the planner's own and differ from the runnability
//! score's.

use crate::RecommendationData;

/// Every KV cache type the planner offers, with its bytes per value.
pub const PLANNER_KV_TYPES: [(&str, f64); 8] = [
    ("f32", 4.0),
    ("f16", 2.0),
    ("q8_0", 1.0),
    ("q5_1", 0.6875),
    ("q5_0", 0.625),
    ("q4_1", 0.5625),
    ("q4_0", 0.5),
    ("iq4_nl", 0.5),
];

/// The planner's bytes per KV value; unknown types count as f16.
#[must_use]
pub fn planner_kv_bytes_per_value(kv_type: &str) -> f64 {
    PLANNER_KV_TYPES
        .iter()
        .find(|(name, _)| *name == kv_type)
        .map_or(2.0, |(_, bytes)| *bytes)
}

fn safety_reserve(total_available: f64) -> f64 {
    (total_available * 0.1).clamp(512_000_000.0, 2_000_000_000.0)
}

fn overhead(model_size: f64) -> f64 {
    (model_size * 0.05).max(200_000_000.0)
}

/// The longest context a file fits in `total_available` bytes with a KV
/// cache of `bytes_per_value`, capped by the model's own maximum; the
/// maximum when the KV size per token is unknown.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the planner computes in doubles and floors, as its formula always did"
)]
pub fn planner_max_context(
    file_size: u64,
    kv_base_per_token: Option<f64>,
    bytes_per_value: f64,
    total_available: u64,
    model_max_context: u64,
    sidecar_reserve_bytes: u64,
) -> u64 {
    let Some(kv_base) = kv_base_per_token.filter(|base| *base > 0.0) else {
        return model_max_context;
    };
    let total = total_available as f64;
    let size = file_size as f64;
    let remaining =
        (total - size - overhead(size) - sidecar_reserve_bytes as f64 - safety_reserve(total))
            .max(0.0);
    let bytes_per_token = kv_base * bytes_per_value;
    if bytes_per_token <= 0.0 {
        return model_max_context;
    }
    let max_context = (remaining / bytes_per_token).floor().max(0.0);
    (max_context as u64).min(model_max_context)
}

/// The layers llama.cpp offloads for a model of `block_count` blocks: every
/// block and the output layer.
#[must_use]
pub fn gpu_offload_layer_count(block_count: Option<u64>) -> Option<u64> {
    block_count
        .filter(|count| *count > 0)
        .map(|count| count + 1)
}

/// Where a downloaded model's layers go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModelOffload {
    #[default]
    Auto,
    Cpu,
    Gpu,
    Mixed,
}

/// The GPU layer count an offload choice stores: none on the CPU, every
/// layer on the GPU, the chosen count when mixed, and no count (llama.cpp
/// decides) otherwise.
#[must_use]
pub fn model_offload_to_gpu_layers(
    offload: ModelOffload,
    total_layers: Option<u64>,
    mixed_layers: Option<u64>,
) -> Option<u64> {
    match offload {
        ModelOffload::Cpu => Some(0),
        ModelOffload::Gpu => total_layers.filter(|layers| *layers > 0),
        ModelOffload::Mixed => mixed_layers.filter(|layers| *layers > 0),
        ModelOffload::Auto => None,
    }
}

/// One file's longest context per planner KV type, in `PLANNER_KV_TYPES`
/// order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerFileLimits {
    pub filename: String,
    pub max_context_by_kv_type: Vec<u64>,
}

/// What the planner shows next to a recommendation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerLimits {
    pub gpu_layer_count: Option<u64>,
    pub files: Vec<PlannerFileLimits>,
}

/// The planner limits of every file `recommendation` lists, with
/// `sidecar_reserve_bytes` held for the projector and draft model.
#[must_use]
pub fn planner_limits(
    recommendation: &RecommendationData,
    sidecar_reserve_bytes: u64,
) -> PlannerLimits {
    PlannerLimits {
        gpu_layer_count: gpu_offload_layer_count(
            recommendation
                .arch
                .as_ref()
                .and_then(|arch| arch.meta.block_count),
        ),
        files: recommendation
            .files
            .iter()
            .map(|file| PlannerFileLimits {
                filename: file.filename.clone(),
                max_context_by_kv_type: PLANNER_KV_TYPES
                    .iter()
                    .map(|(_, bytes_per_value)| {
                        planner_max_context(
                            file.size,
                            recommendation.kv_base_per_token,
                            *bytes_per_value,
                            recommendation.total_available,
                            recommendation.model_max_context,
                            sidecar_reserve_bytes,
                        )
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Where the planner is asked to keep the KV cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlannerKvPlacement {
    #[default]
    Auto,
    Ram,
    Vram,
}

/// Where the planner expects the model and its KV cache to live; unlike the
/// runnability score it can find the GPU unavailable for a GPU choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerGpuMode {
    Full,
    NearFull,
    KvSpill,
    KvHeavySpill,
    RamModelVramCtx,
    RamModelRamCtx,
    MostLayers,
    HalfLayers,
    FewLayers,
    Cpu,
    GpuUnavailable,
}

/// A configuration's score as the planner rates it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannerScore {
    pub score: u32,
    pub label: crate::RunnabilityLabel,
    pub fits_vram: bool,
    pub gpu_mode: PlannerGpuMode,
    pub gpu_score: f64,
}

/// `Math.round` for the non-negative values the planner rounds.
fn round(value: f64) -> f64 {
    (value + 0.5).floor()
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "floored, non-negative context lengths"
)]
fn to_context(value: f64) -> u64 {
    value.max(0.0) as u64
}

/// The longest context whose model, KV cache and buffers all fit in 90% of
/// the VRAM left after the sidecars, the model's maximum once the KV cache
/// reaches the sliding window cap, and 0 below 512 tokens.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "the planner computes in doubles, as its formula always did"
)]
pub fn gpu_optimal_context(
    file_size: u64,
    kv_base_per_token: Option<f64>,
    bytes_per_value: f64,
    available_vram: u64,
    model_max_context: u64,
    kv_context_cap: Option<u64>,
    sidecar_reserve_bytes: u64,
) -> u64 {
    let vram = available_vram as f64;
    let Some(kv_base) = kv_base_per_token.filter(|base| *base != 0.0) else {
        return 0;
    };
    if vram <= 0.0 {
        return 0;
    }
    let size = file_size as f64;
    let budget = (vram * 0.9 - sidecar_reserve_bytes as f64).max(0.0);
    let overhead = overhead(size);
    if size + overhead >= budget {
        return 0;
    }
    let raw = ((budget - size - overhead) / (kv_base * bytes_per_value)).floor();
    capped_context(raw, model_max_context, kv_context_cap)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "the planner computes in doubles, as its formula always did"
)]
fn capped_context(raw: f64, model_max_context: u64, kv_context_cap: Option<u64>) -> u64 {
    if let Some(cap) = kv_context_cap.filter(|cap| *cap != 0)
        && raw >= cap as f64
    {
        return model_max_context;
    }
    if raw >= 512.0 {
        to_context(raw).min(model_max_context)
    } else {
        0
    }
}

/// The longest context that fits in all memory after the sidecars and the
/// safety reserve, with the same caps as `gpu_optimal_context`.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "the planner computes in doubles, as its formula always did"
)]
pub fn ram_max_context(
    file_size: u64,
    kv_base_per_token: Option<f64>,
    bytes_per_value: f64,
    total_available: u64,
    model_max_context: u64,
    kv_context_cap: Option<u64>,
    sidecar_reserve_bytes: u64,
) -> u64 {
    let Some(kv_base) = kv_base_per_token.filter(|base| *base != 0.0) else {
        return 0;
    };
    let total = total_available as f64;
    let size = file_size as f64;
    let remaining =
        (total - size - overhead(size) - sidecar_reserve_bytes as f64 - safety_reserve(total))
            .max(0.0);
    let raw = (remaining / (kv_base * bytes_per_value)).floor();
    capped_context(raw, model_max_context, kv_context_cap)
}

struct Placement {
    score: f64,
    fits_vram: bool,
    gpu_mode: PlannerGpuMode,
    priority: f64,
}

/// The planner's score of one configuration: memory fitness, the placement
/// (the best of GPU, mixed and CPU when the offload is automatic), KV
/// headroom and quantization quality.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the planner's frozen score, kept in one piece with its inputs"
)]
pub fn planner_score(
    model_size: f64,
    quant_quality: f64,
    kv_cache_bytes: f64,
    available_ram: f64,
    total_available: f64,
    available_vram: f64,
    model_offload: ModelOffload,
    kv_placement: PlannerKvPlacement,
    sidecar_reserve_bytes: f64,
) -> PlannerScore {
    let overhead = overhead(model_size);
    let total_needed = model_size + kv_cache_bytes + overhead + sidecar_reserve_bytes;
    let ram_budget = available_ram * 0.9;
    let vram_budget = available_vram * 0.9;
    let model_fits_ram = available_ram > 0.0 && model_size + overhead <= ram_budget;
    let kv_fits_vram = available_vram > 0.0 && kv_cache_bytes + overhead <= vram_budget;
    let kv_fits_ram = available_ram > 0.0 && kv_cache_bytes + overhead <= ram_budget;
    let kv_on_vram = kv_placement == PlannerKvPlacement::Vram;
    let kv_on_ram = kv_placement == PlannerKvPlacement::Ram;
    let effective_vram_need = model_size
        + overhead
        + sidecar_reserve_bytes
        + if kv_on_ram { 0.0 } else { kv_cache_bytes };
    let memory_score = if total_available == 0.0 {
        50.0
    } else if total_needed > total_available {
        0.0
    } else {
        let ratio = total_available / total_needed;
        if ratio < 1.2 {
            20.0
        } else if ratio < 1.5 {
            50.0
        } else if ratio < 2.0 {
            70.0
        } else if ratio < 3.0 {
            85.0
        } else {
            100.0
        }
    };
    let placement_score = |placement: ModelOffload| -> Placement {
        if placement == ModelOffload::Cpu {
            if !model_fits_ram {
                return Placement {
                    score: 0.0,
                    fits_vram: false,
                    gpu_mode: if kv_on_vram && available_vram > 0.0 {
                        PlannerGpuMode::RamModelVramCtx
                    } else {
                        PlannerGpuMode::Cpu
                    },
                    priority: 0.0,
                };
            }
            let ram_fit_ratio = (ram_budget / (model_size + overhead)).min(1.0);
            let base = if kv_on_vram && kv_fits_vram && available_vram > 0.0 {
                82.0
            } else {
                68.0
            };
            return Placement {
                score: base + ram_fit_ratio * if kv_on_vram { 10.0 } else { 14.0 },
                fits_vram: false,
                gpu_mode: if kv_on_vram && available_vram > 0.0 {
                    PlannerGpuMode::RamModelVramCtx
                } else {
                    PlannerGpuMode::RamModelRamCtx
                },
                priority: 0.0,
            };
        }
        let gpu = placement == ModelOffload::Gpu;
        if available_vram <= 0.0 {
            return Placement {
                score: 0.0,
                fits_vram: false,
                gpu_mode: if gpu {
                    PlannerGpuMode::GpuUnavailable
                } else {
                    PlannerGpuMode::Cpu
                },
                priority: if gpu { 2.0 } else { 1.0 },
            };
        }
        let ram_context_fit = || {
            if kv_cache_bytes + overhead > 0.0 {
                (ram_budget / (kv_cache_bytes + overhead)).min(1.0)
            } else {
                1.0
            }
        };
        let spill_mode = if kv_fits_ram {
            PlannerGpuMode::KvSpill
        } else {
            PlannerGpuMode::KvHeavySpill
        };
        if gpu {
            if model_size == 0.0 {
                return Placement {
                    score: 10.0,
                    fits_vram: false,
                    gpu_mode: PlannerGpuMode::GpuUnavailable,
                    priority: 2.0,
                };
            }
            if model_size > vram_budget {
                return Placement {
                    score: 8.0,
                    fits_vram: false,
                    gpu_mode: PlannerGpuMode::GpuUnavailable,
                    priority: 2.0,
                };
            }
            if kv_on_ram {
                return Placement {
                    score: 80.0 + ram_context_fit() * if kv_fits_ram { 12.0 } else { 4.0 },
                    fits_vram: false,
                    gpu_mode: spill_mode,
                    priority: 2.0,
                };
            }
            if effective_vram_need <= vram_budget {
                return Placement {
                    score: 100.0,
                    fits_vram: true,
                    gpu_mode: PlannerGpuMode::Full,
                    priority: 2.0,
                };
            }
            let remaining = vram_budget - model_size;
            let spill = kv_cache_bytes + overhead;
            let fit_ratio = if spill > 0.0 {
                (remaining / spill).min(1.0)
            } else {
                1.0
            };
            let mut adjusted = fit_ratio;
            if kv_on_vram && !kv_fits_vram {
                adjusted *= 0.55;
            }
            if kv_on_ram {
                adjusted = adjusted.max(0.7);
            }
            return Placement {
                score: 72.0 + adjusted * 23.0,
                fits_vram: true,
                gpu_mode: if adjusted >= 0.8 {
                    PlannerGpuMode::NearFull
                } else if adjusted >= 0.4 {
                    PlannerGpuMode::KvSpill
                } else {
                    PlannerGpuMode::KvHeavySpill
                },
                priority: 2.0,
            };
        }
        if kv_on_ram && model_size <= vram_budget {
            return Placement {
                score: (78.0 + ram_context_fit() * if kv_fits_ram { 14.0 } else { 6.0 }).max(12.0),
                fits_vram: false,
                gpu_mode: spill_mode,
                priority: 1.0,
            };
        }
        if effective_vram_need <= vram_budget {
            return Placement {
                score: 96.0,
                fits_vram: true,
                gpu_mode: PlannerGpuMode::Full,
                priority: 1.0,
            };
        }
        if model_size == 0.0 {
            return Placement {
                score: 10.0,
                fits_vram: false,
                gpu_mode: PlannerGpuMode::Cpu,
                priority: 1.0,
            };
        }
        let offload_ratio = (vram_budget / model_size).min(1.0);
        let kv_penalty = if kv_on_vram && !kv_fits_vram {
            12.0
        } else {
            0.0
        };
        let kv_bonus = if kv_on_ram { 4.0 } else { 0.0 };
        Placement {
            score: (28.0 + offload_ratio * 54.0 + kv_bonus - kv_penalty).max(12.0),
            fits_vram: false,
            gpu_mode: if offload_ratio >= 0.75 {
                PlannerGpuMode::MostLayers
            } else if offload_ratio >= 0.5 {
                PlannerGpuMode::HalfLayers
            } else if offload_ratio >= 0.2 {
                PlannerGpuMode::FewLayers
            } else {
                PlannerGpuMode::Cpu
            },
            priority: 1.0,
        }
    };
    let placement = if model_offload == ModelOffload::Auto {
        let mut best: Option<(f64, Placement)> = None;
        for candidate in [ModelOffload::Gpu, ModelOffload::Mixed, ModelOffload::Cpu] {
            let result = placement_score(candidate);
            let adjusted = result.score + result.priority * 4.0;
            if best.as_ref().is_none_or(|(score, _)| adjusted > *score) {
                best = Some((adjusted, result));
            }
        }
        best.map(|(_, placement)| placement)
            .expect("three placements were scored")
    } else {
        placement_score(model_offload)
    };
    let headroom = (total_available - model_size - overhead).max(0.0);
    let kv_score = if kv_cache_bytes == 0.0 {
        50.0
    } else if headroom == 0.0 {
        0.0
    } else if headroom >= kv_cache_bytes {
        let ratio = headroom / kv_cache_bytes;
        if ratio >= 2.0 {
            100.0
        } else {
            50.0 + 50.0 * (ratio - 1.0)
        }
    } else {
        50.0 * (headroom / kv_cache_bytes)
    };
    let mut raw =
        memory_score * 0.25 + placement.score * 0.35 + kv_score * 0.15 + quant_quality * 0.25;
    if memory_score == 0.0 {
        raw = raw.min(10.0);
    }
    let score = round(raw).min(100.0) as u32;
    PlannerScore {
        score,
        label: crate::RunnabilityLabel::from_score(score),
        fits_vram: placement.fits_vram,
        gpu_mode: placement.gpu_mode,
        gpu_score: placement.score,
    }
}

/// A file the planner can choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerFile {
    pub filename: String,
    pub size: u64,
    pub quant_quality: u32,
}

/// What the planner knows of the machine and the model.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannerModel {
    pub available_ram: u64,
    pub available_vram: u64,
    pub total_available: u64,
    pub kv_base_per_token: Option<f64>,
    pub kv_context_cap: Option<u64>,
    pub model_max_context: u64,
    pub block_count: Option<u64>,
    pub supports_gpu_offload: bool,
    pub files: Vec<PlannerFile>,
}

impl PlannerModel {
    #[must_use]
    pub fn from_recommendation(recommendation: &RecommendationData) -> Self {
        Self {
            available_ram: recommendation.available_ram,
            available_vram: recommendation.available_vram,
            total_available: recommendation.total_available,
            kv_base_per_token: recommendation.kv_base_per_token,
            kv_context_cap: recommendation.kv_context_cap,
            model_max_context: recommendation.model_max_context,
            block_count: recommendation
                .arch
                .as_ref()
                .and_then(|arch| arch.meta.block_count),
            supports_gpu_offload: recommendation.supports_gpu_offload,
            files: recommendation
                .files
                .iter()
                .map(|file| PlannerFile {
                    filename: file.filename.clone(),
                    size: file.size,
                    quant_quality: file.quant_quality,
                })
                .collect(),
        }
    }
}

/// What the user picked in the planner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerChoice {
    pub filename: String,
    pub kv_type: String,
    pub model_offload: ModelOffload,
    pub kv_placement: PlannerKvPlacement,
    pub context_length: u64,
    pub sidecar_reserve_bytes: u64,
}

/// How much of the KV cache the planner expects in VRAM and in RAM.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannerKvDistribution {
    pub vram_percent: u32,
    pub on_vram_bytes: f64,
    pub on_ram_bytes: f64,
}

/// A better quantization that still scores 70 or more.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerUpgrade {
    pub filename: String,
    pub score: u32,
}

/// Everything the planner shows for a choice: the context it allows and
/// uses, the memory it needs, its score, the GPU plan, the layer counts and
/// the context the file switch suggests.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannerReport {
    pub filename: String,
    pub max_context: u64,
    pub context_length: u64,
    pub effective_kv_context: u64,
    pub kv_bytes: f64,
    pub overhead_bytes: f64,
    pub total_needed_bytes: f64,
    pub gpu_resident_bytes: f64,
    pub headroom_bytes: f64,
    pub vram_budget_bytes: f64,
    pub score: PlannerScore,
    pub memory_score: u32,
    pub kv_score: u32,
    pub gpu_optimal_context: u64,
    pub ram_max_context: u64,
    pub show_gpu_planning: bool,
    pub offload_percent: u32,
    pub total_layers: Option<u64>,
    pub recommended_layers: Option<u64>,
    /// The longest context that keeps everything in VRAM, when the chosen
    /// one does not.
    pub full_gpu_context: Option<u64>,
    pub kv_distribution: Option<PlannerKvDistribution>,
    pub mixed_gpu_layers: Option<u64>,
    /// The GPU layer count a download with this choice stores.
    pub requested_gpu_layers: Option<u64>,
    pub upgrade: Option<PlannerUpgrade>,
    /// The context the planner sets when this file is picked.
    pub default_context: u64,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "the planner computes in doubles, as its formula always did"
)]
fn kv_bytes(model: &PlannerModel, bytes_per_value: f64, context: u64) -> f64 {
    model
        .kv_base_per_token
        .filter(|base| *base != 0.0)
        .map_or(0.0, |base| base * bytes_per_value * context as f64)
}

fn effective_kv_context(model: &PlannerModel, context: u64) -> u64 {
    match model.kv_context_cap.filter(|cap| *cap != 0) {
        Some(cap) => context.min(cap),
        None => context,
    }
}

/// The planner's report for `choice`; `None` when the model has no files.
#[must_use]
#[expect(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the planner's frozen report, kept in one piece"
)]
pub fn planner_report(model: &PlannerModel, choice: &PlannerChoice) -> Option<PlannerReport> {
    let file = model
        .files
        .iter()
        .find(|file| file.filename == choice.filename)
        .or_else(|| model.files.first())?;
    let bytes_per_value = planner_kv_bytes_per_value(&choice.kv_type);
    let sidecar = choice.sidecar_reserve_bytes;
    let sidecar_bytes = sidecar as f64;
    let total = model.total_available as f64;
    let vram = model.available_vram as f64;
    let size = file.size as f64;
    let max_for = |size: u64| {
        planner_max_context(
            size,
            model.kv_base_per_token,
            bytes_per_value,
            model.total_available,
            model.model_max_context,
            sidecar,
        )
        .max(1024)
    };
    let max_context = max_for(file.size);
    let context_length = choice.context_length.max(1024).min(max_context);
    let effective_kv_context = effective_kv_context(model, context_length);
    let kv_bytes = kv_bytes(model, bytes_per_value, effective_kv_context);
    let overhead = overhead(size);
    let total_needed = size + kv_bytes + overhead + sidecar_bytes;
    let kv_on_ram = choice.kv_placement == PlannerKvPlacement::Ram;
    let gpu_resident = size + overhead + sidecar_bytes + if kv_on_ram { 0.0 } else { kv_bytes };
    let headroom = (total - total_needed).max(0.0);
    let vram_budget = (vram * 0.9 - sidecar_bytes).max(0.0);
    let score = planner_score(
        size,
        f64::from(file.quant_quality),
        kv_bytes,
        model.available_ram as f64,
        total,
        vram,
        choice.model_offload,
        choice.kv_placement,
        sidecar_bytes,
    );
    let gpu_optimal = gpu_optimal_context(
        file.size,
        model.kv_base_per_token,
        bytes_per_value,
        model.available_vram,
        model.model_max_context,
        model.kv_context_cap,
        sidecar,
    );
    let ram_max = ram_max_context(
        file.size,
        model.kv_base_per_token,
        bytes_per_value,
        model.total_available,
        model.model_max_context,
        model.kv_context_cap,
        sidecar,
    );
    let memory_score = if total == 0.0 {
        50
    } else if total_needed > total {
        0
    } else {
        let ratio = total / total_needed;
        if ratio < 1.2 {
            20
        } else if ratio < 1.5 {
            50
        } else if ratio < 2.0 {
            70
        } else if ratio < 3.0 {
            85
        } else {
            100
        }
    };
    let kv_score = if kv_bytes == 0.0 {
        50
    } else {
        let headroom = (total - size - overhead - sidecar_bytes).max(0.0);
        if headroom == 0.0 {
            0
        } else if headroom >= kv_bytes {
            let ratio = headroom / kv_bytes;
            if ratio >= 2.0 {
                100
            } else {
                round(50.0 + 50.0 * (ratio - 1.0)) as u32
            }
        } else {
            round(50.0 * (headroom / kv_bytes)) as u32
        }
    };
    let cpu = choice.model_offload == ModelOffload::Cpu;
    let show_gpu_planning = vram > 0.0 && !cpu;
    let offload_percent = if vram <= 0.0 || cpu {
        0
    } else if choice.model_offload == ModelOffload::Gpu {
        if size <= vram_budget { 100 } else { 0 }
    } else if size <= 0.0 {
        0
    } else if choice.model_offload == ModelOffload::Mixed {
        round(vram_budget / size * 100.0).min(100.0) as u32
    } else if gpu_resident <= vram_budget {
        100
    } else {
        round(vram_budget / gpu_resident * 100.0).min(99.0) as u32
    };
    let total_layers = gpu_offload_layer_count(model.block_count);
    let recommended_layers = total_layers.and_then(|total_layers| {
        if !show_gpu_planning {
            return Some(0);
        }
        if gpu_resident <= vram_budget {
            return Some(total_layers);
        }
        if vram <= 0.0 {
            return None;
        }
        let layers = (vram_budget / gpu_resident * total_layers as f64).floor();
        Some(to_context(layers).min(total_layers))
    });
    let full_gpu_context = model
        .kv_base_per_token
        .filter(|base| *base != 0.0 && vram > 0.0 && !kv_on_ram)
        .and_then(|base| {
            if total_needed <= vram_budget || size + overhead >= vram_budget {
                return None;
            }
            let context = ((vram_budget - size - overhead) / (base * bytes_per_value)).floor();
            (context >= 512.0).then(|| to_context(context))
        });
    let kv_distribution = (kv_bytes > 0.0 && vram > 0.0).then(|| {
        let percent = if (!show_gpu_planning && choice.kv_placement != PlannerKvPlacement::Vram)
            || kv_on_ram
        {
            0.0
        } else if choice.kv_placement == PlannerKvPlacement::Vram {
            round(vram_budget / kv_bytes * 100.0).min(100.0)
        } else if total_needed <= vram_budget {
            100.0
        } else if size >= vram_budget {
            round((vram_budget / size).min(1.0) * 100.0)
        } else {
            let for_kv = vram_budget - size - overhead;
            if for_kv > 0.0 {
                round(for_kv / kv_bytes * 100.0).min(100.0)
            } else {
                0.0
            }
        };
        let on_vram_bytes = kv_bytes * (percent / 100.0);
        PlannerKvDistribution {
            vram_percent: percent as u32,
            on_vram_bytes,
            on_ram_bytes: kv_bytes - on_vram_bytes,
        }
    });
    let mixed_gpu_layers = total_layers
        .filter(|layers| *layers > 0 && vram > 0.0)
        .and_then(|total_layers| {
            if size <= vram_budget {
                return Some(total_layers);
            }
            if total_needed <= 0.0 {
                return None;
            }
            let layer_budget = if kv_on_ram {
                (vram_budget - overhead).max(0.0)
            } else {
                (vram_budget - kv_bytes.min(vram_budget * 0.25)).max(0.0)
            };
            let layers = (layer_budget / total_needed * total_layers as f64).floor();
            Some(to_context(layers).min(total_layers).max(1))
        });
    let requested_offload = if model.supports_gpu_offload {
        choice.model_offload
    } else {
        ModelOffload::Auto
    };
    let requested_gpu_layers =
        model_offload_to_gpu_layers(requested_offload, total_layers, mixed_gpu_layers);
    let upgrade = if file.quant_quality >= 90 {
        None
    } else {
        let mut best: Option<(&PlannerFile, u32)> = None;
        for other in &model.files {
            if other.quant_quality <= file.quant_quality || other.filename == file.filename {
                continue;
            }
            let other_max = max_for(other.size);
            let other_context = context_length.min(other_max);
            let other_kv = kv_bytes_of(model, bytes_per_value, other_context);
            let other_score = planner_score(
                other.size as f64,
                f64::from(other.quant_quality),
                other_kv,
                model.available_ram as f64,
                total,
                vram,
                choice.model_offload,
                choice.kv_placement,
                sidecar_bytes,
            )
            .score;
            if other_score < 70 {
                continue;
            }
            if best.is_none_or(|(best_file, best_score)| {
                other.quant_quality > best_file.quant_quality
                    || (other.quant_quality == best_file.quant_quality && other_score > best_score)
            }) {
                best = Some((other, other_score));
            }
        }
        best.map(|(file, score)| PlannerUpgrade {
            filename: file.filename.clone(),
            score,
        })
    };
    let optimal = if gpu_optimal > 0 {
        gpu_optimal
    } else if ram_max > 0 {
        ram_max
    } else {
        8192
    };
    Some(PlannerReport {
        filename: file.filename.clone(),
        max_context,
        context_length,
        effective_kv_context,
        kv_bytes,
        overhead_bytes: overhead,
        total_needed_bytes: total_needed,
        gpu_resident_bytes: gpu_resident,
        headroom_bytes: headroom,
        vram_budget_bytes: vram_budget,
        score,
        memory_score,
        kv_score,
        gpu_optimal_context: gpu_optimal,
        ram_max_context: ram_max,
        show_gpu_planning,
        offload_percent,
        total_layers,
        recommended_layers,
        full_gpu_context,
        kv_distribution,
        mixed_gpu_layers,
        requested_gpu_layers,
        upgrade,
        default_context: optimal.min(max_context),
    })
}

fn kv_bytes_of(model: &PlannerModel, bytes_per_value: f64, context: u64) -> f64 {
    kv_bytes(model, bytes_per_value, effective_kv_context(model, context))
}

#[cfg(test)]
mod tests {
    use super::*;

    type Case = (u64, Option<f64>, &'static str, u64, u64, u64, u64);

    #[test]
    fn max_context_matches_the_legacy_planner() {
        let cases: [Case; 14] = [
            (
                4_000_000_000,
                Some(131_072.0),
                "q8_0",
                16_000_000_000,
                131_072,
                0,
                77_819,
            ),
            (
                4_000_000_000,
                Some(131_072.0),
                "f16",
                16_000_000_000,
                131_072,
                0,
                38_909,
            ),
            (
                4_000_000_000,
                Some(131_072.0),
                "q4_0",
                16_000_000_000,
                32_768,
                0,
                32_768,
            ),
            (
                8_500_000_000,
                Some(98_304.0),
                "q5_1",
                24_000_000_000,
                1_000_000,
                900_000_000,
                180_146,
            ),
            (
                8_500_000_000,
                Some(98_304.0),
                "iq4_nl",
                24_000_000_000,
                1_000_000,
                900_000_000,
                247_701,
            ),
            (
                8_500_000_000,
                Some(98_304.0),
                "q4_1",
                24_000_000_000,
                1_000_000,
                900_000_000,
                220_178,
            ),
            (
                8_500_000_000,
                Some(98_304.0),
                "q5_0",
                24_000_000_000,
                1_000_000,
                900_000_000,
                198_160,
            ),
            (
                8_500_000_000,
                Some(98_304.0),
                "f32",
                24_000_000_000,
                1_000_000,
                900_000_000,
                30_962,
            ),
            (
                8_500_000_000,
                Some(98_304.0),
                "auto",
                24_000_000_000,
                1_000_000,
                900_000_000,
                61_925,
            ),
            (
                3_000_000_000,
                Some(65_536.0),
                "q8_0",
                4_000_000_000,
                8192,
                0,
                4394,
            ),
            (1_000_000_000, None, "q8_0", 4_000_000_000, 4096, 0, 4096),
            (
                1_000_000_000,
                Some(0.0),
                "q8_0",
                4_000_000_000,
                4096,
                0,
                4096,
            ),
            (
                30_000_000_000,
                Some(262_144.0),
                "q8_0",
                40_000_000_000,
                262_144,
                250_000_000,
                23_841,
            ),
            (
                30_000_000_000,
                Some(262_144.0),
                "q8_0",
                20_000_000_000,
                262_144,
                0,
                0,
            ),
        ];
        for (size, base, kv, total, max, sidecar, expected) in cases {
            assert_eq!(
                planner_max_context(
                    size,
                    base,
                    planner_kv_bytes_per_value(kv),
                    total,
                    max,
                    sidecar
                ),
                expected,
                "{size} {kv} {total} {sidecar}"
            );
        }
    }

    #[test]
    fn layer_counts_match_the_legacy_planner() {
        assert_eq!(gpu_offload_layer_count(Some(32)), Some(33));
        assert_eq!(gpu_offload_layer_count(Some(0)), None);
        assert_eq!(gpu_offload_layer_count(None), None);
        let layers = |offload, total, mixed| model_offload_to_gpu_layers(offload, total, mixed);
        assert_eq!(layers(ModelOffload::Cpu, Some(33), Some(10)), Some(0));
        assert_eq!(layers(ModelOffload::Gpu, Some(33), Some(10)), Some(33));
        assert_eq!(layers(ModelOffload::Gpu, None, Some(10)), None);
        assert_eq!(layers(ModelOffload::Mixed, Some(33), Some(10)), Some(10));
        assert_eq!(layers(ModelOffload::Mixed, Some(33), Some(0)), None);
        assert_eq!(layers(ModelOffload::Auto, Some(33), Some(10)), None);
        assert_eq!(planner_kv_bytes_per_value("q5_1"), 0.6875);
        assert_eq!(crate::kv_bytes_per_value("q5_1"), 0.75);
    }

    #[test]
    fn limits_cover_every_file_and_kv_type() {
        let mut recommendation = RecommendationData::empty();
        recommendation.total_available = 16_000_000_000;
        recommendation.model_max_context = 131_072;
        recommendation.kv_base_per_token = Some(131_072.0);
        recommendation.arch = Some(crate::ModelArchInfo::from(&crate::GgufModelMeta {
            block_count: Some(32),
            ..crate::GgufModelMeta::default()
        }));
        recommendation.files = vec![crate::FileRecommendation {
            filename: "m-Q4_K_M.gguf".to_owned(),
            size: 4_000_000_000,
            quantization: "Q4_K_M".to_owned(),
            quant_quality: 80,
            max_context_f16: 0,
            max_context_q8_0: 0,
            max_context_q4_0: 0,
            optimal_gpu_ctx: 0,
            optimal_ram_ctx: 0,
        }];
        let limits = planner_limits(&recommendation, 0);
        assert_eq!(limits.gpu_layer_count, Some(33));
        assert_eq!(limits.files.len(), 1);
        assert_eq!(
            limits.files[0].max_context_by_kv_type.len(),
            PLANNER_KV_TYPES.len()
        );
        assert_eq!(limits.files[0].max_context_by_kv_type[1], 38_909);
        assert_eq!(limits.files[0].max_context_by_kv_type[2], 77_819);
    }

    fn gpu_mode_name(mode: PlannerGpuMode) -> &'static str {
        match mode {
            PlannerGpuMode::Full => "full",
            PlannerGpuMode::NearFull => "nearFull",
            PlannerGpuMode::KvSpill => "kvSpill",
            PlannerGpuMode::KvHeavySpill => "kvHeavySpill",
            PlannerGpuMode::RamModelVramCtx => "ramModelVramCtx",
            PlannerGpuMode::RamModelRamCtx => "ramModelRamCtx",
            PlannerGpuMode::MostLayers => "mostLayers",
            PlannerGpuMode::HalfLayers => "halfLayers",
            PlannerGpuMode::FewLayers => "fewLayers",
            PlannerGpuMode::Cpu => "cpu",
            PlannerGpuMode::GpuUnavailable => "gpuUnavailable",
        }
    }

    fn offload(name: &str) -> ModelOffload {
        match name {
            "cpu" => ModelOffload::Cpu,
            "gpu" => ModelOffload::Gpu,
            "mixed" => ModelOffload::Mixed,
            _ => ModelOffload::Auto,
        }
    }

    fn placement(name: &str) -> PlannerKvPlacement {
        match name {
            "ram" => PlannerKvPlacement::Ram,
            "vram" => PlannerKvPlacement::Vram,
            _ => PlannerKvPlacement::Auto,
        }
    }

    fn number(value: &serde_json::Value) -> f64 {
        value.as_f64().expect("number")
    }

    fn whole(value: &serde_json::Value) -> u64 {
        value.as_u64().expect("whole number")
    }

    fn score_json(score: &PlannerScore) -> serde_json::Value {
        serde_json::json!({
            "score": score.score,
            "label": score.label.as_str(),
            "fitsVram": score.fits_vram,
            "gpuMode": gpu_mode_name(score.gpu_mode),
            "gpuScore": score.gpu_score,
        })
    }

    /// Equal JSON, numbers compared as doubles.
    fn same(left: &serde_json::Value, right: &serde_json::Value) -> bool {
        use serde_json::Value;
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
            (Value::Object(a), Value::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(key, value)| b.get(key).is_some_and(|other| same(value, other)))
            }
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
            }
            _ => left == right,
        }
    }

    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("../tests/fixtures/legacy_planner.json"))
            .expect("fixture")
    }

    type ContextLimit = fn(u64, Option<f64>, f64, u64, u64, Option<u64>, u64) -> u64;

    #[test]
    fn scores_and_context_limits_match_the_legacy_planner() {
        let fixture = fixture();
        let scores = fixture["scores"].as_array().expect("scores");
        assert_eq!(scores.len(), 60);
        for case in scores {
            let args = case["args"].as_array().expect("args");
            let score = planner_score(
                number(&args[0]),
                number(&args[1]),
                number(&args[2]),
                number(&args[3]),
                number(&args[4]),
                number(&args[5]),
                offload(case["modelOffload"].as_str().expect("offload")),
                placement(case["kvPlacement"].as_str().expect("placement")),
                number(&args[6]),
            );
            assert!(
                same(&score_json(&score), &case["expected"]),
                "{case} gave {}",
                score_json(&score)
            );
        }
        let limits: [(&str, ContextLimit); 2] = [
            ("gpuOptimal", gpu_optimal_context),
            ("ramMax", ram_max_context),
        ];
        for (key, function) in limits {
            for case in fixture[key].as_array().expect("cases") {
                let args = case["args"].as_array().expect("args");
                assert_eq!(
                    function(
                        whole(&args[0]),
                        args[1].as_f64(),
                        number(&args[2]),
                        whole(&args[3]),
                        whole(&args[4]),
                        args[5].as_u64(),
                        whole(&args[6]),
                    ),
                    whole(&case["expected"]),
                    "{key} {case}"
                );
            }
        }
    }

    #[test]
    fn reports_match_the_legacy_planner() {
        let fixture = fixture();
        let plans = fixture["plans"].as_array().expect("plans");
        assert_eq!(plans.len(), 10);
        for case in plans {
            let rec = &case["rec"];
            let model = PlannerModel {
                available_ram: whole(&rec["availableRam"]),
                available_vram: whole(&rec["availableVram"]),
                total_available: whole(&rec["totalAvailable"]),
                kv_base_per_token: rec["kvBasePerToken"].as_f64(),
                kv_context_cap: rec["kvContextCap"].as_u64(),
                model_max_context: whole(&rec["modelMaxContext"]),
                block_count: rec["blockCount"].as_u64(),
                supports_gpu_offload: case["supportsGpuOffload"].as_bool().expect("gpu"),
                files: rec["files"]
                    .as_array()
                    .expect("files")
                    .iter()
                    .map(|file| PlannerFile {
                        filename: file["filename"].as_str().expect("name").to_owned(),
                        size: whole(&file["size"]),
                        quant_quality: u32::try_from(whole(&file["quantQuality"]))
                            .expect("quality"),
                    })
                    .collect(),
            };
            let report = planner_report(
                &model,
                &PlannerChoice {
                    filename: case["file"].as_str().expect("file").to_owned(),
                    kv_type: case["kvType"].as_str().expect("kv").to_owned(),
                    model_offload: offload(case["modelOffload"].as_str().expect("offload")),
                    kv_placement: placement(case["kvPlacement"].as_str().expect("placement")),
                    context_length: whole(&case["contextLength"]),
                    sidecar_reserve_bytes: whole(&case["sidecarReserveBytes"]),
                },
            )
            .expect("report");
            let score = score_json(&report.score);
            let actual = serde_json::json!({
                "maxCtx": report.max_context,
                "clampedCtx": report.context_length,
                "effectiveKvCtx": report.effective_kv_context,
                "kvBytes": report.kv_bytes,
                "overhead": report.overhead_bytes,
                "totalNeeded": report.total_needed_bytes,
                "gpuResidentBytes": report.gpu_resident_bytes,
                "headroom": report.headroom_bytes,
                "vramBudget": report.vram_budget_bytes,
                "score": score["score"],
                "label": score["label"],
                "gpuMode": score["gpuMode"],
                "gpuScore": score["gpuScore"],
                "memoryScore": report.memory_score,
                "kvScore": report.kv_score,
                "detailFullGpuCtx": report.gpu_optimal_context,
                "detailMaxRamCtx": report.ram_max_context,
                "showGpuPlanning": report.show_gpu_planning,
                "offloadPct": report.offload_percent,
                "detailTotalLayers": report.total_layers,
                "detailRecLayers": report.recommended_layers,
                "fullGpuCtx": report.full_gpu_context,
                "kvDistribution": report.kv_distribution.map(|kv| serde_json::json!({
                    "kvVramPct": kv.vram_percent,
                    "kvOnVram": kv.on_vram_bytes,
                    "kvOnRam": kv.on_ram_bytes,
                })),
                "mixed": report.mixed_gpu_layers,
                "requestedGpuLayers": report.requested_gpu_layers,
                "upgrade": report.upgrade.map(|upgrade| serde_json::json!({
                    "filename": upgrade.filename,
                    "score": upgrade.score,
                })),
                "defaultContext": report.default_context,
            });
            assert!(
                same(&actual, &case["expected"]),
                "{}: expected {}\n got {actual}",
                case["name"],
                case["expected"]
            );
        }
    }
}
