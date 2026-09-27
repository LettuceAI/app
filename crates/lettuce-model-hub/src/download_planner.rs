//! The limits the download planner shows while a model is set up before it
//! is downloaded: the longest context each KV cache type allows next to the
//! chosen sidecars, and how many layers llama.cpp can offload. Its bytes
//! per KV value are the planner's own and differ from the scores' for the
//! q5, q4_1 and iq4_nl types.

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
}
