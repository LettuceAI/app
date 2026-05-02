use super::*;
use crate::utils::log_info;
use ort::session::{builder::GraphOptimizationLevel, Session};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokenizers::Tokenizer;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevBenchmarkResult {
    max_tokens_used: u32,
    configured_v4_dimensions: usize,
    v3: BenchmarkVariantResult,
    v4: BenchmarkVariantResult,
    average_speedup_v4_vs_v3: f32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkVariantResult {
    version: String,
    label: String,
    embedding_dimensions: usize,
    sample_count: usize,
    average_ms: f32,
    p95_ms: f32,
    min_ms: f32,
    max_ms: f32,
    related_average: f32,
    unrelated_average: f32,
    separation_margin: f32,
    retrieval_top1: f32,
    retrieval_mrr: f32,
    pair_scores: Vec<BenchmarkPairScore>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkPairScore {
    pair_name: String,
    category: String,
    similarity: f32,
}

struct PairDefinition {
    name: &'static str,
    category: &'static str,
    a: &'static str,
    b: &'static str,
}

struct RetrievalCase {
    name: &'static str,
    query: &'static str,
    relevant_doc_id: &'static str,
}

fn percentile_ms(samples_ms: &[f32], percentile: f32) -> f32 {
    if samples_ms.is_empty() {
        return 0.0;
    }
    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((percentile / 100.0) * (sorted.len().saturating_sub(1) as f32)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn retrieval_corpus() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "lantern-ledger",
            "The Lantern Archive ledger says Mirelle hid the brass key inside the false-bottom tea tin.",
        ),
        (
            "storm-dock",
            "Dockworkers moved the contraband crate to Pier Nine just before the storm reached the harbor.",
        ),
        (
            "garden-letter",
            "A rain-stained letter reveals that the meeting point was the glasshouse behind the manor gardens.",
        ),
        (
            "clocktower-watch",
            "The watch captain changed shifts at the clocktower exactly three minutes before midnight.",
        ),
        (
            "kitchen-rumor",
            "The cook insists the missing signet ring was last seen beside a bowl of pears in the kitchen.",
        ),
        (
            "engine-room",
            "Pressure valves in the engine room were jammed shut with resin, forcing the airship to descend.",
        ),
    ]
}

fn benchmark_pairs() -> Vec<PairDefinition> {
    vec![
        PairDefinition {
            name: "Detail paraphrase",
            category: "related",
            a: "There is one important detail I forgot to mention.",
            b: "I forgot to mention one important detail.",
        },
        PairDefinition {
            name: "Scene paraphrase",
            category: "related",
            a: "The tavern was dimly lit with flickering candles.",
            b: "Candlelight cast shadows across the dark inn.",
        },
        PairDefinition {
            name: "Emotion paraphrase",
            category: "related",
            a: "She felt a wave of sadness wash over her.",
            b: "Her heart ached with sorrow and grief.",
        },
        PairDefinition {
            name: "Memory clue match",
            category: "related",
            a: "The brass key was hidden in the tea tin beneath the false bottom.",
            b: "Mirelle concealed the brass key inside a tea tin with a false base.",
        },
        PairDefinition {
            name: "Science vs food",
            category: "unrelated",
            a: "Quantum mechanics describes subatomic particle behavior.",
            b: "I love eating pizza with extra cheese.",
        },
        PairDefinition {
            name: "Harbor vs kitchen",
            category: "unrelated",
            a: "Dockworkers moved the contraband crate to Pier Nine before the storm.",
            b: "The missing ring was last seen beside a bowl of pears in the kitchen.",
        },
    ]
}

fn retrieval_cases() -> Vec<RetrievalCase> {
    vec![
        RetrievalCase {
            name: "Lantern clue",
            query: "Where did Mirelle hide the brass key?",
            relevant_doc_id: "lantern-ledger",
        },
        RetrievalCase {
            name: "Harbor movement",
            query: "Which pier received the contraband crate before the storm?",
            relevant_doc_id: "storm-dock",
        },
        RetrievalCase {
            name: "Meeting point",
            query: "Where was the secret meeting behind the manor supposed to happen?",
            relevant_doc_id: "garden-letter",
        },
        RetrievalCase {
            name: "Engine failure",
            query: "Why did the airship have to descend unexpectedly?",
            relevant_doc_id: "engine-room",
        },
    ]
}

async fn ensure_benchmark_variant_files(
    app: &AppHandle,
    variant_name: &str,
    target_dir: &Path,
    base_url: &str,
    files: &[&str],
) -> Result<(), String> {
    tokio::fs::create_dir_all(target_dir).await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Failed to create benchmark directory {}: {}",
                target_dir.display(),
                e
            ),
        )
    })?;

    let client = reqwest::Client::new();
    for file in files {
        let target_path = target_dir.join(file);
        if let Some(parent_dir) = target_path.parent() {
            tokio::fs::create_dir_all(parent_dir).await.map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "Failed to create benchmark parent directory {}: {}",
                        parent_dir.display(),
                        e
                    ),
                )
            })?;
        }
        if target_path.exists() {
            continue;
        }

        let url = format!("{}/{}", base_url, file);
        log_info(
            app,
            "embedding_benchmark",
            format!(
                "downloading missing benchmark file variant={} file={} url={}",
                variant_name, file, url
            ),
        );

        let response = client.get(&url).send().await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to download benchmark file {}: {}", file, e),
            )
        })?;
        if !response.status().is_success() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Benchmark download failed for {} (status {})",
                    file,
                    response.status()
                ),
            ));
        }

        let bytes = response.bytes().await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read benchmark file {} bytes: {}", file, e),
            )
        })?;

        let temp_path = target_path.with_extension("tmp");
        tokio::fs::write(&temp_path, &bytes).await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Failed to write benchmark temp file {}: {}",
                    temp_path.display(),
                    e
                ),
            )
        })?;
        tokio::fs::rename(&temp_path, &target_path)
            .await
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "Failed to finalize benchmark file {} -> {}: {}",
                        temp_path.display(),
                        target_path.display(),
                        e
                    ),
                )
            })?;
    }

    Ok(())
}

fn average(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

pub async fn run_embedding_dev_benchmark(app: AppHandle) -> Result<DevBenchmarkResult, String> {
    if !cfg!(debug_assertions) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Embedding benchmark is only available in development builds.",
        ));
    }

    super::ort_runtime::ensure_ort_init(&app).await?;
    let configured_v4_dimensions = resolve_target_embedding_dimensions(
        EmbeddingSourceVersion::V4,
        settings::read_embedding_preferences(&app).embedding_dimensions,
    );

    let model_dir = embedding_model_dir(&app)?;
    let bench_dir = model_dir.join("benchmark");
    let v3_dir = bench_dir.join("v3");
    let v4_dir = bench_dir.join("v4");

    let v3_spec = download_source_spec(Some("v3"));
    ensure_benchmark_variant_files(&app, "v3", &v3_dir, v3_spec.base_url, v3_spec.remote_files)
        .await?;
    let v4_spec = download_source_spec(Some("v4"));
    ensure_benchmark_variant_files(&app, "v4", &v4_dir, v4_spec.base_url, v4_spec.remote_files)
        .await?;

    let benchmark_texts: Vec<&'static str> = vec![
        "The quick brown fox jumps over the lazy dog.",
        "A fast fox leaps over a sleepy canine.",
        "She felt a wave of sadness wash over her.",
        "Her heart ached with sorrow and grief.",
        "The tavern was dimly lit with flickering candles.",
        "Candlelight cast shadows across the dark inn.",
        "Quantum mechanics describes subatomic particle behavior.",
        "I love eating pizza with extra cheese.",
        "There is one important detail I forgot to mention.",
        "I forgot to mention one important detail.",
        "The brass key was hidden in the tea tin beneath the false bottom.",
        "Mirelle concealed the brass key inside a tea tin with a false base.",
        "Pressure valves in the engine room were jammed shut with resin.",
        "Dockworkers moved the contraband crate to Pier Nine before the storm.",
        "The missing ring was last seen beside a bowl of pears in the kitchen.",
    ];
    let pair_definitions = benchmark_pairs();
    let retrieval_docs = retrieval_corpus();
    let retrieval_cases = retrieval_cases();

    let benchmark_future =
        tokio::task::spawn_blocking(move || -> Result<DevBenchmarkResult, String> {
            let run_variant =
                |version: &str,
                 embedding_dimensions: usize,
                 model_path: PathBuf,
                 tokenizer_path: PathBuf|
                 -> Result<BenchmarkVariantResult, String> {
                    let mut session = Session::builder()
                        .map_err(|e| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("Failed to create {} session builder: {}", version, e),
                            )
                        })?
                        .with_optimization_level(GraphOptimizationLevel::Level3)
                        .map_err(|e| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("Failed to set {} optimization level: {}", version, e),
                            )
                        })?
                        .commit_from_file(&model_path)
                        .map_err(|e| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!(
                                    "Failed to load {} model {}: {}",
                                    version,
                                    model_path.display(),
                                    e
                                ),
                            )
                        })?;

                    let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!(
                                "Failed to load {} tokenizer {}: {}",
                                version,
                                tokenizer_path.display(),
                                e
                            ),
                        )
                    })?;

                    let mut embedding_cache: HashMap<String, Vec<f32>> = HashMap::new();

                    for warmup_text in benchmark_texts.iter().take(2) {
                        let _ = super::inference::compute_embedding_with_session(
                            &mut session,
                            &tokenizer,
                            warmup_text,
                            EMBEDDING_BENCH_MAX_SEQ_LENGTH,
                            embedding_dimensions,
                        )?;
                    }

                    let mut timings_ms: Vec<f32> = Vec::with_capacity(benchmark_texts.len());
                    for text in &benchmark_texts {
                        let started = std::time::Instant::now();
                        let embedding = super::inference::compute_embedding_with_session(
                            &mut session,
                            &tokenizer,
                            text,
                            EMBEDDING_BENCH_MAX_SEQ_LENGTH,
                            embedding_dimensions,
                        )?;
                        let elapsed_ms = started.elapsed().as_secs_f32() * 1000.0;
                        timings_ms.push(elapsed_ms);
                        embedding_cache.insert((*text).to_string(), embedding);
                    }

                    for (_, doc_text) in &retrieval_docs {
                        if embedding_cache.contains_key(*doc_text) {
                            continue;
                        }
                        let embedding = super::inference::compute_embedding_with_session(
                            &mut session,
                            &tokenizer,
                            doc_text,
                            EMBEDDING_BENCH_MAX_SEQ_LENGTH,
                            embedding_dimensions,
                        )?;
                        embedding_cache.insert((*doc_text).to_string(), embedding);
                    }

                    for case in &retrieval_cases {
                        if embedding_cache.contains_key(case.query) {
                            continue;
                        }
                        let embedding = super::inference::compute_embedding_with_session(
                            &mut session,
                            &tokenizer,
                            case.query,
                            EMBEDDING_BENCH_MAX_SEQ_LENGTH,
                            embedding_dimensions,
                        )?;
                        embedding_cache.insert(case.query.to_string(), embedding);
                    }

                    let mut pair_scores = Vec::with_capacity(pair_definitions.len());
                    let mut related_scores = Vec::new();
                    let mut unrelated_scores = Vec::new();

                    for pair in &pair_definitions {
                        let e1 = embedding_cache.get(pair.a).ok_or_else(|| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("Missing cached embedding for {}", pair.a),
                            )
                        })?;
                        let e2 = embedding_cache.get(pair.b).ok_or_else(|| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("Missing cached embedding for {}", pair.b),
                            )
                        })?;
                        let similarity = super::util::cosine_similarity(e1, e2);
                        pair_scores.push(BenchmarkPairScore {
                            pair_name: pair.name.to_string(),
                            category: pair.category.to_string(),
                            similarity,
                        });
                        if pair.category == "related" {
                            related_scores.push(similarity);
                        } else {
                            unrelated_scores.push(similarity);
                        }
                    }

                    let mut top1_hits = 0usize;
                    let mut reciprocal_rank_sum = 0.0f32;
                    for case in &retrieval_cases {
                        let query_embedding = embedding_cache.get(case.query).ok_or_else(|| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("Missing cached retrieval query for {}", case.name),
                            )
                        })?;

                        let mut scored_docs: Vec<(&str, f32)> = retrieval_docs
                            .iter()
                            .map(|(doc_id, doc_text)| {
                                let doc_embedding =
                                    embedding_cache.get(*doc_text).expect("retrieval doc cached");
                                (*doc_id, super::util::cosine_similarity(query_embedding, doc_embedding))
                            })
                            .collect();
                        scored_docs.sort_by(|a, b| {
                            b.1.partial_cmp(&a.1)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });

                        if scored_docs
                            .first()
                            .map(|(doc_id, _)| *doc_id == case.relevant_doc_id)
                            .unwrap_or(false)
                        {
                            top1_hits += 1;
                        }

                        if let Some(rank) = scored_docs
                            .iter()
                            .position(|(doc_id, _)| *doc_id == case.relevant_doc_id)
                        {
                            reciprocal_rank_sum += 1.0 / (rank as f32 + 1.0);
                        }
                    }

                    let sample_count = timings_ms.len();
                    let average_ms = average(&timings_ms);
                    let min_ms = timings_ms.iter().copied().reduce(f32::min).unwrap_or(0.0);
                    let max_ms = timings_ms.iter().copied().reduce(f32::max).unwrap_or(0.0);
                    let p95_ms = percentile_ms(&timings_ms, 95.0);
                    let related_average = average(&related_scores);
                    let unrelated_average = average(&unrelated_scores);
                    let retrieval_case_count = retrieval_cases.len().max(1) as f32;

                    Ok(BenchmarkVariantResult {
                        version: version.to_string(),
                        label: format!("{} ({}d)", version, embedding_dimensions),
                        embedding_dimensions,
                        sample_count,
                        average_ms,
                        p95_ms,
                        min_ms,
                        max_ms,
                        related_average,
                        unrelated_average,
                        separation_margin: related_average - unrelated_average,
                        retrieval_top1: top1_hits as f32 / retrieval_case_count,
                        retrieval_mrr: reciprocal_rank_sum / retrieval_case_count,
                        pair_scores,
                    })
                };

            let v3_result = run_variant(
                "v3",
                EMBEDDING_DIM_LEGACY,
                v3_dir.join("model.int8.onnx"),
                v3_dir.join("tokenizer.json"),
            )?;
            let v4_result = run_variant(
                "v4",
                configured_v4_dimensions,
                v4_dir.join("onnx/model.int8.onnx"),
                v4_dir.join("tokenizer.json"),
            )?;

            let average_speedup_v4_vs_v3 = if v4_result.average_ms > 0.0 {
                v3_result.average_ms / v4_result.average_ms
            } else {
                0.0
            };

            Ok(DevBenchmarkResult {
                max_tokens_used: EMBEDDING_BENCH_MAX_SEQ_LENGTH as u32,
                configured_v4_dimensions,
                v3: v3_result,
                v4: v4_result,
                average_speedup_v4_vs_v3,
            })
        });

    benchmark_future.await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Embedding benchmark task failed: {}", e),
        )
    })?
}
