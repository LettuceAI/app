//! The legacy identity, retrieval and separation probes, kept as data.
use crate::MemoryEmbeddingEngine;
use lettuce_embeddings::{EmbeddingRequest, EmbeddingVector};
use lettuce_jobs::handle::CancellationToken;
use serde::Deserialize;

#[derive(Deserialize)]
struct CorpusEntry {
    id: String,
    text: String,
}
#[derive(Deserialize)]
struct RetrievalCase {
    name: String,
    query: String,
    expected_id: String,
}
#[derive(Deserialize)]
struct Pair {
    name: String,
    a: String,
    b: String,
}
#[derive(Deserialize)]
struct Probes {
    identity_probe: String,
    corpus: Vec<CorpusEntry>,
    cases: Vec<RetrievalCase>,
    related: Vec<Pair>,
    unrelated: Vec<Pair>,
}

#[derive(Debug)]
pub(crate) struct HealthCase {
    pub name: String,
    pub rank: usize,
    pub top_score: f32,
    pub correct_score: f32,
}

#[derive(Debug)]
pub(crate) struct HealthReport {
    pub identity_cosine: f32,
    pub top1_rate: f32,
    pub top3_rate: f32,
    pub mrr: f32,
    pub related_avg: f32,
    pub unrelated_avg: f32,
    pub cases: Vec<HealthCase>,
}

impl HealthReport {
    pub(crate) fn passed(&self) -> bool {
        self.identity_cosine >= 0.9990
            && self.top1_rate >= 0.60
            && self.related_avg - self.unrelated_avg >= 0.10
    }
}

pub(crate) fn run(
    engine: &dyn MemoryEmbeddingEngine,
    cancel: &CancellationToken,
) -> Result<(), String> {
    if !report(engine, cancel)?.passed() {
        return Err("the installed embedding model failed the legacy health check".into());
    }
    Ok(())
}

pub(crate) fn report(
    engine: &dyn MemoryEmbeddingEngine,
    cancel: &CancellationToken,
) -> Result<HealthReport, String> {
    let probes: Probes =
        serde_json::from_str(include_str!("../../resources/embedding-health/v1.json"))
            .map_err(|_| "invalid embedding health probes")?;
    let dimensions = engine.dimensions();
    let embed = |text: &str| -> Result<EmbeddingVector, String> {
        if cancel.is_cancelled() {
            return Err("embedding health check cancelled".into());
        }
        let vector = engine
            .embed_memory(
                &EmbeddingRequest {
                    text: text.to_owned(),
                    dimensions,
                },
                cancel,
            )
            .map_err(|_| "embedding health inference failed")?;
        if vector.values.len() != dimensions.get()
            || vector.source_revision != engine.source_revision()
            || vector.values.iter().any(|value| !value.is_finite())
        {
            return Err("embedding health returned an invalid vector".into());
        }
        Ok(vector)
    };
    let cosine = |left: &EmbeddingVector, right: &EmbeddingVector| {
        left.cosine_similarity(right)
            .filter(|value| value.is_finite())
            .ok_or_else(|| "embedding health returned an invalid cosine".to_owned())
    };
    let identity = cosine(
        &embed(&probes.identity_probe)?,
        &embed(&probes.identity_probe)?,
    )?;
    let docs = probes
        .corpus
        .iter()
        .map(|entry| embed(&entry.text))
        .collect::<Result<Vec<_>, _>>()?;
    if probes.cases.is_empty() {
        return Err("empty embedding health cases".into());
    }
    let mut cases = Vec::with_capacity(probes.cases.len());
    for case in &probes.cases {
        let query = embed(&case.query)?;
        let mut scored = docs
            .iter()
            .enumerate()
            .map(|(index, doc)| cosine(&query, doc).map(|score| (index, score)))
            .collect::<Result<Vec<_>, _>>()?;
        scored.sort_by(|left, right| right.1.total_cmp(&left.1));
        let best = scored.first().ok_or("empty embedding health corpus")?;
        let rank = scored
            .iter()
            .position(|(index, _)| probes.corpus[*index].id == case.expected_id)
            .ok_or("embedding health case has no expected document")?
            + 1;
        cases.push(HealthCase {
            name: case.name.clone(),
            rank,
            top_score: best.1,
            correct_score: scored[rank - 1].1,
        });
        tracing::debug!(case = %case.name, expected = %case.expected_id, actual = %probes.corpus[best.0].id, "embedding health retrieval");
    }
    let mean = |pairs: &[Pair]| -> Result<f32, String> {
        if pairs.is_empty() {
            return Err("empty embedding health pairs".into());
        }
        let mut total = 0.0;
        for pair in pairs {
            let score = cosine(&embed(&pair.a)?, &embed(&pair.b)?)?;
            tracing::debug!(pair = %pair.name, score, "embedding health separation");
            total += score;
        }
        Ok(total / pairs.len() as f32)
    };
    let top1 = cases.iter().filter(|case| case.rank == 1).count() as f32 / cases.len() as f32;
    let top3 = cases.iter().filter(|case| case.rank <= 3).count() as f32 / cases.len() as f32;
    let mrr = cases.iter().map(|case| 1.0 / case.rank as f32).sum::<f32>() / cases.len() as f32;
    let related_avg = mean(&probes.related)?;
    let unrelated_avg = mean(&probes.unrelated)?;
    let margin = related_avg - unrelated_avg;
    tracing::info!(identity, top1, margin, "embedding health check");
    Ok(HealthReport {
        identity_cosine: identity,
        top1_rate: top1,
        top3_rate: top3,
        mrr,
        related_avg,
        unrelated_avg,
        cases,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    struct ProbeEngine {
        values: std::collections::HashMap<String, usize>,
        constant: bool,
    }
    impl MemoryEmbeddingEngine for ProbeEngine {
        fn source_revision(&self) -> &str {
            "health-test"
        }
        fn dimensions(&self) -> lettuce_embeddings::EmbeddingDimensions {
            lettuce_embeddings::EmbeddingDimensions::D128
        }
        fn count_tokens(&self, _text: &str) -> Result<u32, crate::EmbeddingGenerationError> {
            Ok(1)
        }
        fn embed_memory(
            &self,
            request: &EmbeddingRequest,
            _cancel: &CancellationToken,
        ) -> Result<EmbeddingVector, crate::EmbeddingGenerationError> {
            let mut values = vec![0.0; 128];
            let index = if self.constant {
                0
            } else {
                *self.values.get(&request.text).expect("known probe")
            };
            values[index] = 1.0;
            Ok(EmbeddingVector {
                source_revision: self.source_revision().into(),
                values,
            })
        }
    }
    pub(crate) fn engine(constant: bool) -> std::sync::Arc<dyn MemoryEmbeddingEngine> {
        let probes: Probes =
            serde_json::from_str(include_str!("../../resources/embedding-health/v1.json"))
                .expect("probes");
        let mut values = std::collections::HashMap::new();
        values.insert(probes.identity_probe.clone(), 100);
        for (index, entry) in probes.corpus.iter().enumerate() {
            values.insert(entry.text.clone(), index);
        }
        for (index, pair) in probes.related.iter().enumerate() {
            values.insert(pair.a.clone(), 60 + index);
            values.insert(pair.b.clone(), 60 + index);
        }
        for (index, pair) in probes.unrelated.iter().enumerate() {
            values.insert(pair.a.clone(), 70 + index * 2);
            values.insert(pair.b.clone(), 71 + index * 2);
        }
        for case in &probes.cases {
            let entry = probes
                .corpus
                .iter()
                .find(|entry| entry.id == case.expected_id)
                .expect("expected");
            values.insert(
                case.query.clone(),
                *values.get(&entry.text).expect("corpus value"),
            );
        }
        std::sync::Arc::new(ProbeEngine { values, constant })
    }
    #[test]
    fn health_checks_retrieval_and_separation_and_honors_cancellation() {
        let engine = engine(false);
        run(engine.as_ref(), &CancellationToken::new()).expect("all three checks pass");
        let bad = self::engine(true);
        assert!(
            run(bad.as_ref(), &CancellationToken::new())
                .expect_err("identity alone is insufficient")
                .contains("failed the legacy health check")
        );
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(
            run(bad.as_ref(), &cancel)
                .expect_err("cancelled")
                .contains("cancelled")
        );
    }
    #[test]
    fn diagnostics_retains_measured_health_and_every_retrieval_rank() {
        let engine = engine(false);
        let report = report(engine.as_ref(), &CancellationToken::new()).expect("measured report");
        assert_eq!(report.identity_cosine, 1.0);
        assert_eq!(report.top1_rate, 1.0);
        assert_eq!(report.top3_rate, 1.0);
        assert_eq!(report.mrr, 1.0);
        assert_eq!(report.related_avg, 1.0);
        assert_eq!(report.unrelated_avg, 0.0);
        assert_eq!(report.cases.len(), 50);
        assert!(report.cases.iter().all(|case| {
            !case.name.is_empty()
                && case.rank == 1
                && case.top_score == 1.0
                && case.correct_score == 1.0
        }));
        let bad = self::engine(true);
        let failure =
            super::report(bad.as_ref(), &CancellationToken::new()).expect("measured failure");
        assert!(!failure.passed());
        assert_eq!(failure.related_avg, 1.0);
        assert_eq!(failure.unrelated_avg, 1.0);
    }

    #[test]
    fn legacy_health_dataset_is_complete() {
        let probes: Probes =
            serde_json::from_str(include_str!("../../resources/embedding-health/v1.json"))
                .expect("probes");
        assert_eq!(probes.corpus.len(), 50);
        assert_eq!(probes.cases.len(), 50);
        assert_eq!(probes.related.len(), 4);
        assert_eq!(probes.unrelated.len(), 4);
        for case in probes.cases {
            assert!(
                probes
                    .corpus
                    .iter()
                    .any(|entry| entry.id == case.expected_id)
            );
        }
    }
}
