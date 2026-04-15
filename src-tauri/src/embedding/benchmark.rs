use super::*;
use crate::utils::log_info;
use ort::session::{builder::GraphOptimizationLevel, Session};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tokenizers::Tokenizer;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevBenchmarkResult {
    max_tokens_used: u32,
    v2: BenchmarkVariantResult,
    v3: BenchmarkVariantResult,
    pair_deltas: Vec<BenchmarkPairDelta>,
    average_speedup_v3_vs_v2: f32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkVariantResult {
    version: String,
    sample_count: usize,
    average_ms: f32,
    p95_ms: f32,
    min_ms: f32,
    max_ms: f32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkPairDelta {
    pair_name: String,
    v2_similarity: f32,
    v3_similarity: f32,
    delta: f32,
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

pub async fn run_embedding_dev_benchmark(app: AppHandle) -> Result<DevBenchmarkResult, String> {
    if !cfg!(debug_assertions) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Embedding benchmark is only available in development builds.",
        ));
    }

    super::ort_runtime::ensure_ort_init(&app).await?;
    let model_dir = embedding_model_dir(&app)?;
    let bench_dir = model_dir.join("benchmark");
    let v2_dir = bench_dir.join("v2");
    let v3_dir = bench_dir.join("v3");

    let v2_spec = download_source_spec(Some("v2"));
    ensure_benchmark_variant_files(&app, "v2", &v2_dir, v2_spec.base_url, v2_spec.remote_files)
        .await?;
    let v3_spec = download_source_spec(Some("v3"));
    ensure_benchmark_variant_files(&app, "v3", &v3_dir, v3_spec.base_url, v3_spec.remote_files)
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
    ];
    let comparison_pairs: Vec<(&'static str, &'static str, &'static str)> = vec![
        (
            "Detail paraphrase",
            "There is one important detail I forgot to mention.",
            "I forgot to mention one important detail.",
        ),
        (
            "Scene paraphrase",
            "The tavern was dimly lit with flickering candles.",
            "Candlelight cast shadows across the dark inn.",
        ),
        (
            "Unrelated sample",
            "Quantum mechanics describes subatomic particle behavior.",
            "I love eating pizza with extra cheese.",
        ),
    ];

    let benchmark_future =
        tokio::task::spawn_blocking(move || -> Result<DevBenchmarkResult, String> {
            let run_variant =
                |version: &str,
                 model_path: PathBuf,
                 tokenizer_path: PathBuf|
                 -> Result<(BenchmarkVariantResult, HashMap<String, f32>), String> {
                    let model_bytes = fs::read(&model_path).map_err(|e| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!(
                                "Failed to read {} model {}: {}",
                                version,
                                model_path.display(),
                                e
                            ),
                        )
                    })?;
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
                        .commit_from_memory(&model_bytes)
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

                    for warmup_text in benchmark_texts.iter().take(2) {
                        let _ = super::inference::compute_embedding_with_session(
                            &mut session,
                            &tokenizer,
                            warmup_text,
                            EMBEDDING_BENCH_MAX_SEQ_LENGTH,
                        )?;
                    }

                    let mut timings_ms: Vec<f32> = Vec::with_capacity(benchmark_texts.len());
                    let mut embedding_cache: HashMap<String, Vec<f32>> =
                        HashMap::with_capacity(benchmark_texts.len());

                    for text in &benchmark_texts {
                        let started = std::time::Instant::now();
                        let embedding = super::inference::compute_embedding_with_session(
                            &mut session,
                            &tokenizer,
                            text,
                            EMBEDDING_BENCH_MAX_SEQ_LENGTH,
                        )?;
                        let elapsed_ms = started.elapsed().as_secs_f32() * 1000.0;
                        timings_ms.push(elapsed_ms);
                        embedding_cache.insert((*text).to_string(), embedding);
                    }

                    let sample_count = timings_ms.len();
                    let average_ms = if sample_count == 0 {
                        0.0
                    } else {
                        timings_ms.iter().sum::<f32>() / sample_count as f32
                    };
                    let min_ms = timings_ms.iter().copied().reduce(f32::min).unwrap_or(0.0);
                    let max_ms = timings_ms.iter().copied().reduce(f32::max).unwrap_or(0.0);
                    let p95_ms = percentile_ms(&timings_ms, 95.0);

                    let mut pair_scores: HashMap<String, f32> = HashMap::new();
                    for (pair_name, a, b) in &comparison_pairs {
                        let e1 = embedding_cache.get(*a).ok_or_else(|| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("Missing cached embedding for pair text A ({})", pair_name),
                            )
                        })?;
                        let e2 = embedding_cache.get(*b).ok_or_else(|| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("Missing cached embedding for pair text B ({})", pair_name),
                            )
                        })?;
                        pair_scores.insert(
                            (*pair_name).to_string(),
                            super::util::cosine_similarity(e1, e2),
                        );
                    }

                    Ok((
                        BenchmarkVariantResult {
                            version: version.to_string(),
                            sample_count,
                            average_ms,
                            p95_ms,
                            min_ms,
                            max_ms,
                        },
                        pair_scores,
                    ))
                };

            let (v2_result, v2_pairs) = run_variant(
                "v2",
                v2_dir.join("model.onnx"),
                v2_dir.join("tokenizer.json"),
            )?;
            let (v3_result, v3_pairs) = run_variant(
                "v3",
                v3_dir.join("model.int8.onnx"),
                v3_dir.join("tokenizer.json"),
            )?;

            let mut pair_deltas: Vec<BenchmarkPairDelta> = Vec::new();
            for (pair_name, _, _) in &comparison_pairs {
                let v2_similarity = *v2_pairs.get(*pair_name).unwrap_or(&0.0);
                let v3_similarity = *v3_pairs.get(*pair_name).unwrap_or(&0.0);
                pair_deltas.push(BenchmarkPairDelta {
                    pair_name: (*pair_name).to_string(),
                    v2_similarity,
                    v3_similarity,
                    delta: v3_similarity - v2_similarity,
                });
            }

            let average_speedup_v3_vs_v2 = if v3_result.average_ms > 0.0 {
                v2_result.average_ms / v3_result.average_ms
            } else {
                0.0
            };

            Ok(DevBenchmarkResult {
                max_tokens_used: EMBEDDING_BENCH_MAX_SEQ_LENGTH as u32,
                v2: v2_result,
                v3: v3_result,
                pair_deltas,
                average_speedup_v3_vs_v2,
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
