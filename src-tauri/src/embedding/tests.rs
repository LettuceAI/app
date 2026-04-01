use super::*;
use crate::utils::{log_error, log_info};
use ort::session::{builder::GraphOptimizationLevel, Session};
use std::fs;
use tauri::Emitter;
use tokenizers::Tokenizer;
use tokio::time::{timeout, Duration};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    success: bool,
    message: String,
    scores: Vec<ScoreComparison>,
    model_info: ModelTestInfo,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelTestInfo {
    version: String,
    max_tokens: u32,
    embedding_dimensions: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ScoreComparison {
    pair_name: String,
    text_a: String,
    text_b: String,
    similarity_score: f32,
    expected: String,
    passed: bool,
    category: String,
}

pub async fn run_embedding_test(app: AppHandle) -> Result<TestResult, String> {
    log_info(&app, "embedding_test", "starting embedding test");
    log_info(&app, "embedding_test", "Starting embedding test...");

    let test_cases: Vec<(&str, &str, &str, &str, f32, &str)> = vec![
        (
            "Semantic: Animal Description",
            "The quick brown fox jumps over the lazy dog",
            "A fast fox leaps over a sleepy canine",
            "semantic",
            0.6,
            "High similarity expected - same meaning, different words",
        ),
        (
            "Semantic: Weather",
            "It's raining heavily outside today",
            "There's a big storm with lots of precipitation",
            "semantic",
            0.5,
            "High similarity expected - related weather concepts",
        ),
        (
            "Semantic: Greeting",
            "Hello, how are you doing today?",
            "Hi there, how's it going?",
            "semantic",
            0.6,
            "High similarity expected - same intent",
        ),
        (
            "Dissimilar: Fox vs Physics",
            "The quick brown fox jumps over the lazy dog",
            "Quantum mechanics describes subatomic particle behavior",
            "dissimilar",
            0.5,
            "Low similarity expected - unrelated topics",
        ),
        (
            "Dissimilar: Food vs Technology",
            "I love eating pizza with extra cheese",
            "The computer crashed and lost all my files",
            "dissimilar",
            0.5,
            "Low similarity expected - unrelated topics",
        ),
        (
            "Roleplay: Emotional State",
            "She felt a wave of sadness wash over her",
            "Her heart ached with sorrow and grief",
            "roleplay",
            0.55,
            "High similarity expected - same emotional content",
        ),
        (
            "Roleplay: Action Description",
            "He drew his sword and charged at the enemy",
            "The warrior unsheathed his blade and rushed forward to attack",
            "roleplay",
            0.55,
            "High similarity expected - same action described differently",
        ),
        (
            "Roleplay: Setting",
            "The tavern was dimly lit with flickering candles",
            "Candlelight cast shadows across the dark inn",
            "roleplay",
            0.5,
            "High similarity expected - similar scene description",
        ),
    ];

    let total_tests = test_cases.len();
    let _ = app.emit(
        "embedding_test_progress",
        serde_json::json!({
            "current": 0,
            "total": total_tests,
            "stage": "starting"
        }),
    );

    let app_for_test = app.clone();
    super::ort_runtime::ensure_ort_init(&app_for_test).await?;

    let test_future = tokio::task::spawn_blocking(move || {
        let mut scores: Vec<ScoreComparison> = Vec::new();
        let mut all_passed = true;
        let mut embedding_dim = 0;

        let detected_version = detect_model_version(&app_for_test)?;
        log_info(
            &app_for_test,
            "embedding_test",
            format!("detected model version {:?}", detected_version),
        );

        let (_selected_source, model_path, tokenizer_path, max_seq_length, version_label) =
            resolve_runtime_model(&app_for_test)?;

        log_info(&app_for_test, "embedding_test", "ort initialized");

        let model_bytes = fs::read(&model_path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read {} model {}: {}", version_label, model_path.display(), e),
            )
        })?;
        let mut session = Session::builder()
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to create session builder: {}", e),
                )
            })?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to set optimization level: {}", e),
                )
            })?
            .commit_from_memory(&model_bytes)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to load {} model: {}", version_label, e),
                )
            })?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load tokenizer: {}", e),
            )
        })?;

        for (idx, (name, text_a, text_b, category, threshold, expected_desc)) in
            test_cases.iter().enumerate()
        {
            log_info(&app_for_test, "embedding_test", format!("testing {}", name));
            log_info(
                &app_for_test,
                "embedding_test",
                format!("Testing: {}", name),
            );

            let emb_a = super::inference::compute_embedding_with_session(
                &mut session,
                &tokenizer,
                text_a,
                max_seq_length,
            )
            .map_err(|e| {
                log_error(
                    &app_for_test,
                    "embedding_test",
                    format!("failed to embed {} text_a error={}", name, e),
                );
                format!("Failed to embed '{}': {}", name, e)
            })?;

            if embedding_dim == 0 {
                embedding_dim = emb_a.len();
                log_info(
                    &app_for_test,
                    "embedding_test",
                    format!("embedding dimension set to {}", embedding_dim),
                );
            }

            let emb_b = super::inference::compute_embedding_with_session(
                &mut session,
                &tokenizer,
                text_b,
                max_seq_length,
            )
            .map_err(|e| {
                log_error(
                    &app_for_test,
                    "embedding_test",
                    format!("failed to embed {} text_b error={}", name, e),
                );
                format!("Failed to embed '{}': {}", name, e)
            })?;

            let similarity = super::util::cosine_similarity(&emb_a, &emb_b);

            let passed = if *category == "dissimilar" {
                similarity < *threshold
            } else {
                similarity >= *threshold
            };

            if !passed {
                all_passed = false;
            }

            log_info(
                &app_for_test,
                "embedding_test",
                format!(
                    "result name={} category={} similarity={} threshold={} passed={}",
                    name, category, similarity, threshold, passed
                ),
            );

            scores.push(ScoreComparison {
                pair_name: (*name).to_string(),
                text_a: (*text_a).to_string(),
                text_b: (*text_b).to_string(),
                similarity_score: similarity,
                expected: (*expected_desc).to_string(),
                passed,
                category: (*category).to_string(),
            });

            let _ = app_for_test.emit(
                "embedding_test_progress",
                serde_json::json!({
                    "current": idx + 1,
                    "total": total_tests,
                    "stage": "running"
                }),
            );
        }

        let model_info = get_embedding_model_info(app_for_test.clone())?;

        let passed_count = scores.iter().filter(|s| s.passed).count();
        let total_count = scores.len();
        log_info(
            &app_for_test,
            "embedding_test",
            format!(
                "embedding test complete passed={} total={} all_passed={}",
                passed_count, total_count, all_passed
            ),
        );

        let message = if all_passed {
            format!(
                "All {} tests passed! The embedding model is working correctly.",
                total_count
            )
        } else {
            format!(
                "{}/{} tests passed. Some results were unexpected - the model may need reinstallation.",
                passed_count, total_count
            )
        };

        Ok(TestResult {
            success: all_passed,
            message,
            scores,
            model_info: ModelTestInfo {
                version: model_info
                    .source_version
                    .or(model_info.version)
                    .unwrap_or_else(|| "unknown".to_string()),
                max_tokens: model_info.max_tokens,
                embedding_dimensions: embedding_dim,
            },
        })
    });

    let result = timeout(
        Duration::from_secs(EMBEDDING_TEST_TIMEOUT_SECS),
        test_future,
    )
    .await
    .map_err(|_| "Embedding test timed out. Please try again.".to_string())?
    .map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Embedding test failed to start: {}", e),
        )
    })?;

    let _ = app.emit(
        "embedding_test_progress",
        serde_json::json!({
            "current": total_tests,
            "total": total_tests,
            "stage": "completed"
        }),
    );

    result
}

pub async fn compare_custom_texts(
    app: AppHandle,
    text_a: String,
    text_b: String,
) -> Result<f32, String> {
    if text_a.trim().is_empty() || text_b.trim().is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Both texts must be non-empty",
        ));
    }

    log_info(
        &app,
        "embedding_test",
        format!(
            "compare custom texts len_a={} len_b={}",
            text_a.len(),
            text_b.len()
        ),
    );

    let emb_a = super::inference::compute_embedding(app.clone(), text_a)
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to embed first text: {}", e),
            )
        })?;

    let emb_b = super::inference::compute_embedding(app.clone(), text_b)
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to embed second text: {}", e),
            )
        })?;

    Ok(super::util::cosine_similarity(&emb_a, &emb_b))
}
