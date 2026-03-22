use super::*;
use crate::utils::log_info;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use crate::utils::log_warn;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use ort::{
    execution_providers::coreml::{
        CoreMLComputeUnits, CoreMLExecutionProvider, CoreMLModelFormat,
        CoreMLSpecializationStrategy,
    },
    execution_providers::ExecutionProviderDispatch,
};
use ort::{
    inputs,
    session::{builder::GraphOptimizationLevel, Session},
    value::Value,
};
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use tauri::Emitter;
use tokenizers::Tokenizer;

struct LoadedEmbeddingRuntime {
    model_path: PathBuf,
    tokenizer_path: PathBuf,
    max_seq_length: usize,
    session: Session,
    tokenizer: Tokenizer,
}

lazy_static::lazy_static! {
    static ref LOADED_EMBEDDING_RUNTIME: Arc<TokioMutex<Option<LoadedEmbeddingRuntime>>> =
        Arc::new(TokioMutex::new(None));
}

#[cfg(target_os = "macos")]
static COREML_FALLBACK_TOAST_SHOWN: AtomicBool = AtomicBool::new(false);

pub async fn clear_loaded_runtime_cache() {
    let mut cache = LOADED_EMBEDDING_RUNTIME.lock().await;
    *cache = None;
}

fn describe_path(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(meta) => format!(
            "exists=true file={} dir={} size_bytes={}",
            meta.is_file(),
            meta.is_dir(),
            meta.len()
        ),
        Err(err) => format!("exists=false error={}", err),
    }
}

fn log_model_file_status(app: &AppHandle, component: &str, model_dir: &PathBuf) {
    for filename in MODEL_FILES_V1.iter() {
        let path = model_dir.join(filename);
        log_info(
            app,
            component,
            format!("model file v1 {}: {}", filename, describe_path(&path)),
        );
    }

    for filename in MODEL_FILES_V2_LOCAL.iter() {
        let path = model_dir.join(filename);
        log_info(
            app,
            component,
            format!("model file v2 {}: {}", filename, describe_path(&path)),
        );
    }
    for filename in MODEL_FILES_V3_LOCAL.iter() {
        let path = model_dir.join(filename);
        log_info(
            app,
            component,
            format!("model file v3 {}: {}", filename, describe_path(&path)),
        );
    }
}

pub(super) fn compute_embedding_with_session(
    session: &mut Session,
    tokenizer: &Tokenizer,
    text: &str,
    max_seq_length: usize,
) -> Result<Vec<f32>, String> {
    let encoding = tokenizer.encode(text, true).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Tokenization failed: {}", e),
        )
    })?;

    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();

    let seq_len = input_ids.len().min(max_seq_length);
    let input_ids = &input_ids[..seq_len];
    let attention_mask = &attention_mask[..seq_len];

    let input_ids_i64: Vec<i64> = input_ids.iter().map(|&x| x as i64).collect();
    let attention_mask_i64: Vec<i64> = attention_mask.iter().map(|&x| x as i64).collect();
    let type_ids = encoding.get_type_ids();
    let token_type_ids_i64: Vec<i64> = if type_ids.len() >= seq_len {
        type_ids[..seq_len].iter().map(|&x| x as i64).collect()
    } else {
        vec![0; seq_len]
    };

    let run_without_token_type_ids = |session: &mut Session| -> Result<Vec<f32>, String> {
        let input_ids_value =
            Value::from_array(([1, seq_len], input_ids_i64.clone())).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to create input_ids tensor: {}", e),
                )
            })?;
        let attention_mask_value = Value::from_array(([1, seq_len], attention_mask_i64.clone()))
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to create attention_mask tensor: {}", e),
                )
            })?;

        let outputs = session
            .run(inputs![
                "input_ids" => input_ids_value,
                "attention_mask" => attention_mask_value
            ])
            .map_err(|e| {
                crate::utils::err_msg(module_path!(), line!(), format!("Inference failed: {}", e))
            })?;

        let embedding_value = &outputs[0];
        let (_, embedding_slice) = embedding_value.try_extract_tensor::<f32>().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to extract embedding: {}", e),
            )
        })?;
        Ok(embedding_slice.to_vec())
    };

    let run_with_token_type_ids = |session: &mut Session| -> Result<Vec<f32>, String> {
        let input_ids_value =
            Value::from_array(([1, seq_len], input_ids_i64.clone())).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to create input_ids tensor: {}", e),
                )
            })?;
        let attention_mask_value = Value::from_array(([1, seq_len], attention_mask_i64.clone()))
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to create attention_mask tensor: {}", e),
                )
            })?;
        let token_type_ids_value = Value::from_array(([1, seq_len], token_type_ids_i64.clone()))
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to create token_type_ids tensor: {}", e),
                )
            })?;

        let outputs = session
            .run(inputs![
                "input_ids" => input_ids_value,
                "attention_mask" => attention_mask_value,
                "token_type_ids" => token_type_ids_value
            ])
            .map_err(|e| {
                crate::utils::err_msg(module_path!(), line!(), format!("Inference failed: {}", e))
            })?;

        let embedding_value = &outputs[0];
        let (_, embedding_slice) = embedding_value.try_extract_tensor::<f32>().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to extract embedding: {}", e),
            )
        })?;
        Ok(embedding_slice.to_vec())
    };

    let expects_token_type_ids = session
        .inputs
        .iter()
        .any(|input| input.name.contains("token_type_ids"));

    let embedding_vec = if expects_token_type_ids {
        run_with_token_type_ids(session)?
    } else {
        match run_without_token_type_ids(session) {
            Ok(vec) => vec,
            Err(err) if err.contains("Missing Input: token_type_ids") => {
                run_with_token_type_ids(session).map_err(|fallback_err| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!(
                        "Inference failed after token_type_ids fallback: {} (initial error: {})",
                        fallback_err, err
                    ),
                    )
                })?
            }
            Err(err) => return Err(err),
        }
    };

    match embedding_vec.len() {
        len if len == EMBEDDING_DIM => Ok(embedding_vec),
        len if len > EMBEDDING_DIM && len % EMBEDDING_DIM == 0 => {
            Ok(embedding_vec[..EMBEDDING_DIM].to_vec())
        }
        len => Err(format!(
            "Unexpected embedding dimension: {} (expected {} or multiple thereof)",
            len, EMBEDDING_DIM
        )),
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn configure_session_builder_for_target(
    builder: ort::session::builder::SessionBuilder,
    model_path: &Path,
) -> Result<ort::session::builder::SessionBuilder, String> {
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    {
        let mut builder = builder;
        let cache_dir = model_path
            .parent()
            .ok_or_else(|| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    "Failed to resolve parent directory for embedding model path",
                )
            })?
            .join("coreml-cache");

        fs::create_dir_all(&cache_dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        // Prefer Apple acceleration; ORT falls back to CPU for unsupported ops.
        let coreml_ep: ExecutionProviderDispatch = CoreMLExecutionProvider::default()
            .with_compute_units(CoreMLComputeUnits::CPUAndNeuralEngine)
            .with_model_format(CoreMLModelFormat::MLProgram)
            .with_specialization_strategy(CoreMLSpecializationStrategy::FastPrediction)
            .with_static_input_shapes(true)
            .with_model_cache_dir(cache_dir.to_string_lossy())
            .build()
            .error_on_failure();

        builder = builder.with_execution_providers([coreml_ep]).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to configure CoreML execution provider: {}", e),
            )
        })?;

        return Ok(builder);
    }

    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    {
        let _ = model_path;
        Ok(builder)
    }
}

fn create_base_session_builder() -> Result<ort::session::builder::SessionBuilder, String> {
    Session::builder()
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
        })
}

fn create_runtime(
    app: &AppHandle,
    model_path: &Path,
    tokenizer_path: &Path,
) -> Result<(Session, Tokenizer), String> {
    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    let _ = app;

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    let session_builder =
        match configure_session_builder_for_target(create_base_session_builder()?, model_path) {
            Ok(builder) => builder,
            Err(err) => {
                log_warn(
                    app,
                    "embedding_debug",
                    format!(
                        "CoreML execution provider unavailable; falling back to CPU: {}",
                        err
                    ),
                );
                #[cfg(target_os = "macos")]
                {
                    if COREML_FALLBACK_TOAST_SHOWN
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        let _ = app.emit(
                        "app://toast",
                        serde_json::json!({
                            "variant": "warning",
                            "title": "Embedding acceleration fallback",
                            "description": "CoreML is unavailable. Embeddings are running on CPU."
                        }),
                    );
                    }
                }
                create_base_session_builder()?
            }
        };

    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    let session_builder = create_base_session_builder()?;

    let session = session_builder.commit_from_file(model_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to load model: {}", e),
        )
    })?;
    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to load tokenizer from {:?}: {}", tokenizer_path, e),
        )
    })?;
    Ok((session, tokenizer))
}

pub async fn compute_embedding(app: AppHandle, text: String) -> Result<Vec<f32>, String> {
    let text_len = text.len();
    crate::utils::log_info(
        &app,
        "embedding_debug",
        format!(
            "computing embedding for text_len_bytes={} text='{}'",
            text_len, text
        ),
    );

    let model_dir = embedding_model_dir(&app)?;
    log_info(
        &app,
        "embedding_debug",
        format!(
            "model_dir={} {}",
            model_dir.display(),
            describe_path(&model_dir)
        ),
    );

    let (selected_source, model_path, tokenizer_path, max_seq_length, version_label) =
        resolve_runtime_model(&app)?;
    log_info(
        &app,
        "embedding_debug",
        format!("selected_embedding_source={}", selected_source.as_str()),
    );
    log_info(
        &app,
        "embedding_debug",
        format!(
            "embedding model version={} model_path={} tokenizer_path={}",
            version_label,
            model_path.display(),
            tokenizer_path.display()
        ),
    );
    log_model_file_status(&app, "embedding_debug", &model_dir);
    log_info(
        &app,
        "embedding_debug",
        format!("model_path status {}", describe_path(&model_path)),
    );
    log_info(
        &app,
        "embedding_debug",
        format!("tokenizer_path status {}", describe_path(&tokenizer_path)),
    );

    super::ort_runtime::ensure_ort_init(&app).await?;
    log_info(&app, "embedding_debug", "ort initialized");

    let keep_model_loaded = settings::read_embedding_preferences(&app).keep_model_loaded;
    let embedding_vec = if keep_model_loaded {
        let mut cache = LOADED_EMBEDDING_RUNTIME.lock().await;
        let reuse = cache.as_ref().is_some_and(|loaded| {
            loaded.model_path == model_path
                && loaded.tokenizer_path == tokenizer_path
                && loaded.max_seq_length == max_seq_length
        });

        if !reuse {
            let (session, tokenizer) = create_runtime(&app, &model_path, &tokenizer_path)?;
            *cache = Some(LoadedEmbeddingRuntime {
                model_path: model_path.clone(),
                tokenizer_path: tokenizer_path.clone(),
                max_seq_length,
                session,
                tokenizer,
            });
            log_info(
                &app,
                "embedding_debug",
                format!(
                    "created persistent embedding runtime model={}",
                    model_path.display()
                ),
            );
        }

        let loaded = cache.as_mut().ok_or_else(|| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                "Embedding runtime cache unavailable",
            )
        })?;
        log_info(
            &app,
            "embedding_debug",
            "running embedding inference (persistent runtime)",
        );
        compute_embedding_with_session(
            &mut loaded.session,
            &loaded.tokenizer,
            &text,
            loaded.max_seq_length,
        )?
    } else {
        {
            let mut cache = LOADED_EMBEDDING_RUNTIME.lock().await;
            if cache.is_some() {
                *cache = None;
                log_info(
                    &app,
                    "embedding_debug",
                    "cleared persistent embedding runtime cache",
                );
            }
        }

        let (mut session, tokenizer) = create_runtime(&app, &model_path, &tokenizer_path)?;
        log_info(
            &app,
            "embedding_debug",
            format!("onnx session ready model_path={}", model_path.display()),
        );
        log_info(&app, "embedding_debug", "tokenizer loaded");
        log_info(&app, "embedding_debug", "running embedding inference");
        compute_embedding_with_session(&mut session, &tokenizer, &text, max_seq_length)?
    };
    log_info(
        &app,
        "embedding_debug",
        format!("embedding extracted len={}", embedding_vec.len()),
    );
    Ok(embedding_vec)
}

pub async fn initialize_embedding_model(app: AppHandle) -> Result<(), String> {
    let detected_version = detect_model_version(&app)?;
    log_info(
        &app,
        "embedding_init",
        format!("initialize embedding model version={:?}", detected_version),
    );
    if detected_version.is_none() {
        crate::utils::log_error(&app, "embedding_init", "model files not found");
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Model files not found. Please download the model first.",
        ));
    }

    let (_, model_path, tokenizer_path, max_seq_length, version_label) =
        resolve_runtime_model(&app)?;

    if !model_path.exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Model file missing: {}", model_path.display()),
        ));
    }
    if !tokenizer_path.exists() {
        return Err(format!(
            "Tokenizer file missing: {}",
            tokenizer_path.display()
        ));
    }

    let model_size = fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0);
    let tokenizer_size = fs::metadata(&tokenizer_path).map(|m| m.len()).unwrap_or(0);
    if model_size == 0 || tokenizer_size == 0 {
        return Err(format!(
            "Model files look invalid (sizes: model={} bytes, tokenizer={} bytes)",
            model_size, tokenizer_size
        ));
    }

    super::ort_runtime::ensure_ort_init(&app).await?;
    log_info(&app, "embedding_init", "ort initialized");

    let keep_model_loaded = settings::read_embedding_preferences(&app).keep_model_loaded;
    if keep_model_loaded {
        let mut cache = LOADED_EMBEDDING_RUNTIME.lock().await;
        let reuse = cache.as_ref().is_some_and(|loaded| {
            loaded.model_path == model_path && loaded.tokenizer_path == tokenizer_path
        });
        if !reuse {
            let (session, tokenizer) = create_runtime(&app, &model_path, &tokenizer_path)?;
            *cache = Some(LoadedEmbeddingRuntime {
                model_path: model_path.clone(),
                tokenizer_path: tokenizer_path.clone(),
                max_seq_length,
                session,
                tokenizer,
            });
            log_info(
                &app,
                "embedding_init",
                format!(
                    "persistent embedding runtime primed model={} tokenizer={}",
                    model_path.display(),
                    tokenizer_path.display()
                ),
            );
        }
        return Ok(());
    }

    let _session = Session::builder()
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
        .commit_from_file(&model_path)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load {} model: {}", version_label, e),
            )
        })?;
    let _tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
        format!(
            "Failed to load tokenizer from {}: {}",
            tokenizer_path.display(),
            e
        )
    })?;

    log_info(
        &app,
        "embedding_init",
        format!(
            "model validation ok version={} model_path={} tokenizer_path={}",
            version_label,
            model_path.display(),
            tokenizer_path.display()
        ),
    );

    Ok(())
}
