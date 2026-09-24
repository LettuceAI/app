use std::collections::BTreeMap;

use serde::Deserialize;

use crate::EmbeddingDimensions;

const DIMENSIONS: [EmbeddingDimensions; 5] = [
    EmbeddingDimensions::D64,
    EmbeddingDimensions::D128,
    EmbeddingDimensions::D256,
    EmbeddingDimensions::D512,
    EmbeddingDimensions::D768,
];
const MAX_CALIBRATION_BYTES: usize = 64 * 1024;

/// How a model's raw cosine similarity becomes the score thresholds and
/// people see. v4 thresholds were tuned on raw cosine; Eidos publishes a
/// per-dimension linear map, `clamp(a * cosine + b, 0, 1)`, with the
/// thresholds that apply after it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimilarityCalibration {
    RawCosine,
    Linear(LinearSimilarityCalibration),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearSimilarityCalibration {
    maps: [(f32, f32); 5],
    default_threshold: f32,
    fallback_threshold: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SimilarityCalibrationError {
    #[error("similarity calibration is not valid JSON of the expected shape")]
    Malformed,
    #[error("similarity calibration lacks a valid map for {0} dimensions")]
    MissingDimension(usize),
    #[error("similarity calibration thresholds are invalid")]
    InvalidThresholds,
}

#[derive(Deserialize)]
struct CalibrationFile {
    default_threshold: f64,
    fallback_threshold: f64,
    dims: BTreeMap<String, CalibrationMap>,
}

#[derive(Deserialize)]
struct CalibrationMap {
    a: f64,
    b: f64,
}

const fn slot(dimensions: EmbeddingDimensions) -> usize {
    match dimensions {
        EmbeddingDimensions::D64 => 0,
        EmbeddingDimensions::D128 => 1,
        EmbeddingDimensions::D256 => 2,
        EmbeddingDimensions::D512 => 3,
        EmbeddingDimensions::D768 => 4,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn finite_f32(value: f64) -> Option<f32> {
    let narrowed = value as f32;
    (value.is_finite() && narrowed.is_finite()).then_some(narrowed)
}

impl SimilarityCalibration {
    /// Parses a published `calibration.json`. Every dimension the app can
    /// store needs a map with a positive slope; entries for other dimensions
    /// are ignored.
    pub fn from_json(bytes: &[u8]) -> Result<Self, SimilarityCalibrationError> {
        if bytes.len() > MAX_CALIBRATION_BYTES {
            return Err(SimilarityCalibrationError::Malformed);
        }
        let file: CalibrationFile =
            serde_json::from_slice(bytes).map_err(|_| SimilarityCalibrationError::Malformed)?;
        let mut maps = [(0.0, 0.0); 5];
        for dimensions in DIMENSIONS {
            let map = file
                .dims
                .get(&dimensions.get().to_string())
                .and_then(|map| Some((finite_f32(map.a)?, finite_f32(map.b)?)))
                .filter(|(a, _)| *a > 0.0)
                .ok_or(SimilarityCalibrationError::MissingDimension(
                    dimensions.get(),
                ))?;
            maps[slot(dimensions)] = map;
        }
        let default_threshold = finite_f32(file.default_threshold)
            .ok_or(SimilarityCalibrationError::InvalidThresholds)?;
        let fallback_threshold = finite_f32(file.fallback_threshold)
            .ok_or(SimilarityCalibrationError::InvalidThresholds)?;
        if !(0.0 < fallback_threshold
            && fallback_threshold <= default_threshold
            && default_threshold <= 1.0)
        {
            return Err(SimilarityCalibrationError::InvalidThresholds);
        }
        Ok(Self::Linear(LinearSimilarityCalibration {
            maps,
            default_threshold,
            fallback_threshold,
        }))
    }

    /// The shown score for a raw cosine between two vectors of `dimensions`.
    /// Raw cosine passes through unchanged; a NaN stays NaN so it still
    /// fails every threshold.
    #[must_use]
    pub fn score(&self, cosine: f32, dimensions: EmbeddingDimensions) -> f32 {
        match self {
            Self::RawCosine => cosine,
            Self::Linear(linear) => {
                let (a, b) = linear.maps[slot(dimensions)];
                (a * cosine + b).clamp(0.0, 1.0)
            }
        }
    }

    /// The published default and fallback thresholds on the shown scale;
    /// `None` for raw cosine, whose thresholds come from settings.
    #[must_use]
    pub const fn published_thresholds(&self) -> Option<(f32, f32)> {
        match self {
            Self::RawCosine => None,
            Self::Linear(linear) => Some((linear.default_threshold, linear.fallback_threshold)),
        }
    }

    /// The minimum retrieval score. A configured value is compared with the
    /// shown score as-is. Without one, raw cosine uses `unset_raw_cosine`,
    /// and a published calibration its default threshold, or its permissive
    /// fallback when no candidate reaches the default.
    #[must_use]
    pub fn retrieval_threshold(
        &self,
        configured: Option<f32>,
        unset_raw_cosine: f32,
        scores: impl IntoIterator<Item = f32>,
    ) -> f32 {
        match (configured, self.published_thresholds()) {
            (Some(configured), _) => configured,
            (None, None) => unset_raw_cosine,
            (None, Some((default, fallback))) => {
                if scores.into_iter().any(|score| score >= default) {
                    default
                } else {
                    fallback
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EIDOS: &str = r#"{
        "formula": "clamp(a * cosine + b, 0, 1)",
        "default_threshold": 0.5,
        "fallback_threshold": 0.35,
        "dims": {
            "768": {"a": 2.381, "b": -1.381, "raw_default": 0.79},
            "512": {"a": 2.2901, "b": -1.2863},
            "384": {"a": 2.2556, "b": -1.2549},
            "256": {"a": 2.2388, "b": -1.244},
            "128": {"a": 2.2388, "b": -1.2664},
            "64": {"a": 1.9481, "b": -1.0058}
        }
    }"#;

    fn eidos() -> SimilarityCalibration {
        SimilarityCalibration::from_json(EIDOS.as_bytes()).expect("calibration")
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 1e-4
    }

    #[test]
    fn published_eidos_calibration_maps_and_clamps_per_dimension() {
        let calibration = eidos();
        assert!(close(
            calibration.score(0.79, EmbeddingDimensions::D768),
            2.381 * 0.79 - 1.381
        ));
        assert!(close(
            calibration.score(0.79, EmbeddingDimensions::D768),
            0.49999
        ));
        assert!(close(
            calibration.score(0.732, EmbeddingDimensions::D768),
            0.3619
        ));
        assert_eq!(calibration.score(0.4, EmbeddingDimensions::D768), 0.0);
        assert_eq!(calibration.score(1.0, EmbeddingDimensions::D768), 1.0);
        assert!(close(
            calibration.score(0.78, EmbeddingDimensions::D512),
            2.2901 * 0.78 - 1.2863
        ));
        assert!(close(
            calibration.score(0.7, EmbeddingDimensions::D64),
            1.9481 * 0.7 - 1.0058
        ));
        assert!(close(
            calibration.score(0.8, EmbeddingDimensions::D128),
            2.2388 * 0.8 - 1.2664
        ));
        assert_eq!(calibration.score(0.2, EmbeddingDimensions::D256), 0.0);
        assert!(
            calibration
                .score(f32::NAN, EmbeddingDimensions::D768)
                .is_nan()
        );
        assert_eq!(calibration.published_thresholds(), Some((0.5, 0.35)));
    }

    #[test]
    fn raw_cosine_keeps_scores_and_the_configured_threshold() {
        let raw = SimilarityCalibration::RawCosine;
        assert_eq!(raw.score(0.42, EmbeddingDimensions::D768), 0.42);
        assert_eq!(raw.score(-0.3, EmbeddingDimensions::D64), -0.3);
        assert_eq!(raw.retrieval_threshold(None, 0.35, [0.9]), 0.35);
        assert_eq!(raw.retrieval_threshold(Some(0.42), 0.35, [0.9]), 0.42);
        assert_eq!(raw.published_thresholds(), None);
    }

    #[test]
    fn unset_retrieval_uses_the_default_threshold_then_the_fallback_and_a_set_one_as_is() {
        let calibration = eidos();
        assert_eq!(
            calibration.retrieval_threshold(None, 0.35, [0.1, 0.55]),
            0.5
        );
        assert_eq!(
            calibration.retrieval_threshold(None, 0.35, [0.1, 0.4]),
            0.35
        );
        assert_eq!(calibration.retrieval_threshold(None, 0.35, []), 0.35);
        assert_eq!(
            calibration.retrieval_threshold(Some(0.6), 0.35, [0.1, 0.4]),
            0.6
        );
        assert_eq!(calibration.retrieval_threshold(Some(0.2), 0.35, [0.9]), 0.2);
    }

    #[test]
    fn incomplete_or_inconsistent_calibration_is_refused() {
        let missing = EIDOS.replace("\"64\"", "\"65\"");
        assert_eq!(
            SimilarityCalibration::from_json(missing.as_bytes()),
            Err(SimilarityCalibrationError::MissingDimension(64))
        );
        let flat = EIDOS.replace("\"a\": 2.381", "\"a\": 0.0");
        assert_eq!(
            SimilarityCalibration::from_json(flat.as_bytes()),
            Err(SimilarityCalibrationError::MissingDimension(768))
        );
        let inverted = EIDOS.replace(
            "\"fallback_threshold\": 0.35",
            "\"fallback_threshold\": 0.6",
        );
        assert_eq!(
            SimilarityCalibration::from_json(inverted.as_bytes()),
            Err(SimilarityCalibrationError::InvalidThresholds)
        );
        assert_eq!(
            SimilarityCalibration::from_json(b"[]"),
            Err(SimilarityCalibrationError::Malformed)
        );
    }
}
