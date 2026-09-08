use std::{
    collections::HashMap,
    ffi::c_void,
    fmt,
    sync::{Arc, Mutex},
};

use lettuce_jobs::handle::CancellationToken;
use lettuce_model_hub::{VerifiedWhisperArtifacts, WhisperModelRepository};
use lettuce_types::ContentHash;
use tracing::{debug, warn};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, get_lang_str,
    install_logging_hooks,
};

use crate::{
    AsrModelDescriptor, AsrRuntime, AsrRuntimeError, MAX_SEGMENTS, RuntimeTranscription,
    TranscriptionOptions, TranscriptionSegment,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct WhisperContextKey {
    artifact_hash: ContentHash,
    use_gpu: bool,
    flash_attention: bool,
    gpu_device: i32,
}

pub struct WhisperCppRuntime<R: ?Sized> {
    repository: Arc<R>,
    contexts: Mutex<HashMap<WhisperContextKey, Arc<WhisperContext>>>,
}

impl<R: ?Sized> fmt::Debug for WhisperCppRuntime<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WhisperCppRuntime")
            .field(
                "repository",
                &format_args!("<{}>", std::any::type_name::<R>()),
            )
            .finish_non_exhaustive()
    }
}

impl<R: ?Sized> WhisperCppRuntime<R> {
    #[must_use]
    pub fn new(repository: Arc<R>) -> Self {
        install_logging_hooks();
        Self {
            repository,
            contexts: Mutex::new(HashMap::new()),
        }
    }

    pub fn clear_cache(&self) -> Result<usize, AsrRuntimeError> {
        let mut contexts = self
            .contexts
            .lock()
            .map_err(|_| AsrRuntimeError::Unavailable)?;
        let count = contexts.len();
        contexts.clear();
        Ok(count)
    }
}

impl<R: WhisperModelRepository + ?Sized> WhisperCppRuntime<R> {
    pub fn preload(
        &self,
        model: &AsrModelDescriptor,
        options: &TranscriptionOptions,
    ) -> Result<(), AsrRuntimeError> {
        options.validate().map_err(|_| AsrRuntimeError::Rejected)?;
        let artifacts = self.resolve_verified(model)?;
        let key = context_key(&artifacts, options);
        let _ = self.context(&artifacts, &key, true)?;
        Ok(())
    }

    fn resolve_verified(
        &self,
        model: &AsrModelDescriptor,
    ) -> Result<VerifiedWhisperArtifacts, AsrRuntimeError> {
        let manifest = self
            .repository
            .get_whisper_model(model.id.as_str())
            .map_err(|_| AsrRuntimeError::Unavailable)?
            .ok_or(AsrRuntimeError::ModelUnavailable)?;
        let artifacts = manifest
            .verify()
            .map_err(|_| AsrRuntimeError::ModelUnavailable)?;
        if artifacts.model_id != model.id.as_str()
            || artifacts.blake3 != model.artifact_hash
            || artifacts.english_only != model.english_only
        {
            return Err(AsrRuntimeError::ModelUnavailable);
        }
        Ok(artifacts)
    }

    fn context(
        &self,
        artifacts: &VerifiedWhisperArtifacts,
        key: &WhisperContextKey,
        retain: bool,
    ) -> Result<Arc<WhisperContext>, AsrRuntimeError> {
        if retain {
            if let Some(context) = self
                .contexts
                .lock()
                .map_err(|_| AsrRuntimeError::Unavailable)?
                .get(key)
                .cloned()
            {
                debug!(
                    model_id = artifacts.model_id,
                    "reusing Whisper model context"
                );
                return Ok(context);
            }
        }
        let mut parameters = WhisperContextParameters::new();
        parameters
            .use_gpu(key.use_gpu)
            .flash_attn(key.flash_attention)
            .gpu_device(key.gpu_device);
        debug!(
            model_id = artifacts.model_id,
            use_gpu = key.use_gpu,
            flash_attention = key.flash_attention,
            gpu_device = key.gpu_device,
            "loading verified Whisper model context"
        );
        let context = Arc::new(
            WhisperContext::new_with_params(&artifacts.model_path, parameters).map_err(
                |error| {
                    warn!(model_id = artifacts.model_id, %error, "failed to load Whisper model");
                    AsrRuntimeError::ModelUnavailable
                },
            )?,
        );
        if !retain {
            return Ok(context);
        }
        let mut contexts = self
            .contexts
            .lock()
            .map_err(|_| AsrRuntimeError::Unavailable)?;
        Ok(contexts
            .entry(key.clone())
            .or_insert_with(|| context.clone())
            .clone())
    }
}

impl<R: WhisperModelRepository + ?Sized> AsrRuntime for WhisperCppRuntime<R> {
    fn transcribe(
        &self,
        model: &AsrModelDescriptor,
        mono_16khz: &[f32],
        prompt: &str,
        options: &TranscriptionOptions,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeTranscription, AsrRuntimeError> {
        if cancellation.is_cancelled() {
            return Err(AsrRuntimeError::Cancelled);
        }
        options.validate().map_err(|_| AsrRuntimeError::Rejected)?;
        if mono_16khz.is_empty()
            || mono_16khz.iter().any(|sample| !sample.is_finite())
            || prompt.contains('\0')
        {
            return Err(AsrRuntimeError::Rejected);
        }
        let artifacts = self.resolve_verified(model)?;
        if cancellation.is_cancelled() {
            return Err(AsrRuntimeError::Cancelled);
        }
        let key = context_key(&artifacts, options);
        let context = self.context(&artifacts, &key, options.keep_model_loaded)?;
        if cancellation.is_cancelled() {
            return Err(AsrRuntimeError::Cancelled);
        }
        let mut state = context.create_state().map_err(|error| {
            warn!(model_id = artifacts.model_id, %error, "failed to create Whisper state");
            AsrRuntimeError::Failed
        })?;
        let language = options.language.as_deref().map(str::to_ascii_lowercase);
        let mut params = build_params(options, language.as_deref(), prompt);
        let callback_token = Box::new(cancellation.clone());
        unsafe {
            params.set_abort_callback(Some(whisper_should_abort));
            params.set_abort_callback_user_data(
                std::ptr::from_ref(callback_token.as_ref())
                    .cast_mut()
                    .cast(),
            );
        }
        let transcription = state.full(params, mono_16khz);
        drop(callback_token);
        if let Err(error) = transcription {
            if cancellation.is_cancelled() {
                return Err(AsrRuntimeError::Cancelled);
            }
            warn!(model_id = artifacts.model_id, %error, "Whisper transcription failed");
            return Err(AsrRuntimeError::Failed);
        }
        if cancellation.is_cancelled() {
            return Err(AsrRuntimeError::Cancelled);
        }
        let segments = collect_segments(&state)?;
        let raw_text = segments
            .iter()
            .map(|segment| segment.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let detected_language = get_lang_str(state.full_lang_id_from_state()).map(str::to_owned);
        let result = RuntimeTranscription {
            raw_text,
            detected_language,
            segments,
        };
        result.validate().map_err(|_| AsrRuntimeError::Failed)?;
        Ok(result)
    }
}

unsafe extern "C" fn whisper_should_abort(user_data: *mut c_void) -> bool {
    if user_data.is_null() {
        return true;
    }
    unsafe { &*user_data.cast::<CancellationToken>() }.is_cancelled()
}

fn context_key(
    artifacts: &VerifiedWhisperArtifacts,
    options: &TranscriptionOptions,
) -> WhisperContextKey {
    WhisperContextKey {
        artifact_hash: artifacts.blake3.clone(),
        use_gpu: options.use_gpu && !options.force_cpu,
        flash_attention: options.flash_attention,
        gpu_device: options.gpu_device,
    }
}

fn build_params<'a>(
    options: &TranscriptionOptions,
    language: Option<&'a str>,
    prompt: &'a str,
) -> FullParams<'a, 'static> {
    let mut params = FullParams::new(SamplingStrategy::Greedy {
        best_of: options.best_of.unwrap_or(1).max(1),
    });
    params.set_n_threads(
        i32::try_from(options.threads.unwrap_or(4).max(1)).expect("validated thread count"),
    );
    params.set_translate(options.translate);
    params.set_no_context(options.no_context);
    params.set_single_segment(options.single_segment);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_suppress_nst(false);
    if let Some(value) = options.offset_ms {
        params.set_offset_ms(value.max(0));
    }
    if let Some(value) = options.duration_ms {
        params.set_duration_ms(value.max(0));
    }
    if let Some(value) = options.max_len {
        params.set_max_len(value.max(0));
    }
    if let Some(value) = options.max_tokens {
        params.set_max_tokens(value.max(0));
    }
    if let Some(value) = options.temperature {
        params.set_temperature(value);
    }
    if let Some(value) = options.temperature_inc {
        params.set_temperature_inc(value);
    }
    params.set_token_timestamps(options.token_timestamps);
    params.set_split_on_word(options.split_on_word);
    let detect_language =
        options.detect_language || language.is_some_and(|value| value.eq_ignore_ascii_case("auto"));
    if detect_language {
        params.set_language(None);
        params.set_detect_language(true);
    } else {
        params.set_language(language);
    }
    if !prompt.is_empty() {
        params.set_initial_prompt(prompt);
    }
    params
}

fn collect_segments(
    state: &whisper_rs::WhisperState,
) -> Result<Vec<TranscriptionSegment>, AsrRuntimeError> {
    let count = usize::try_from(state.full_n_segments()).map_err(|_| AsrRuntimeError::Failed)?;
    if count > MAX_SEGMENTS {
        return Err(AsrRuntimeError::Failed);
    }
    (0..count)
        .map(|index| {
            let native_index = i32::try_from(index).map_err(|_| AsrRuntimeError::Failed)?;
            let segment = state
                .get_segment(native_index)
                .ok_or(AsrRuntimeError::Failed)?;
            let start_ms = u64::try_from(segment.start_timestamp().saturating_mul(10))
                .map_err(|_| AsrRuntimeError::Failed)?;
            let end_ms = u64::try_from(segment.end_timestamp().saturating_mul(10))
                .map_err(|_| AsrRuntimeError::Failed)?;
            Ok(TranscriptionSegment {
                index: u32::try_from(index).map_err(|_| AsrRuntimeError::Failed)?,
                start_ms,
                end_ms,
                text: segment
                    .to_str_lossy()
                    .map_err(|_| AsrRuntimeError::Failed)?
                    .into_owned(),
                no_speech_probability: segment.no_speech_probability(),
                speaker_turn_next: segment.next_segment_speaker_turn(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use lettuce_model_hub::{InstalledWhisperManifest, WhisperModelRepositoryError};
    use lettuce_types::{OperationId, TimestampMillis};

    use super::*;

    #[derive(Debug)]
    struct Repository {
        manifest: InstalledWhisperManifest,
    }

    impl WhisperModelRepository for Repository {
        fn admit_whisper_model(
            &self,
            _: InstalledWhisperManifest,
        ) -> Result<InstalledWhisperManifest, WhisperModelRepositoryError> {
            Err(WhisperModelRepositoryError::Conflict)
        }

        fn get_whisper_model(
            &self,
            model_id: &str,
        ) -> Result<Option<InstalledWhisperManifest>, WhisperModelRepositoryError> {
            Ok((self.manifest.model_id == model_id).then(|| self.manifest.clone()))
        }

        fn list_whisper_models(
            &self,
        ) -> Result<Vec<InstalledWhisperManifest>, WhisperModelRepositoryError> {
            Ok(vec![self.manifest.clone()])
        }
    }

    fn fixture(path: &Path) -> (Arc<Repository>, AsrModelDescriptor) {
        let root = path.join("models");
        let folder = root.join("tiny.en");
        std::fs::create_dir_all(&folder).expect("model directory");
        let model_path = folder.join("ggml-tiny.en.bin");
        std::fs::write(&model_path, b"invalid deterministic Whisper model").expect("model file");
        let manifest =
            InstalledWhisperManifest::inspect_legacy(&root, &model_path, TimestampMillis::new(10))
                .expect("manifest");
        let descriptor = AsrModelDescriptor {
            id: crate::AsrModelId::new(manifest.model_id.clone()).expect("model id"),
            artifact_hash: manifest.model.blake3.clone(),
            english_only: manifest.english_only,
        };
        (Arc::new(Repository { manifest }), descriptor)
    }

    #[test]
    fn invokes_the_native_loader_only_after_artifact_verification() {
        let root = std::env::temp_dir().join(format!("whisper-runtime-{}", OperationId::new()));
        let (repository, descriptor) = fixture(&root);
        let runtime = WhisperCppRuntime::new(repository);
        let error = runtime
            .preload(
                &descriptor,
                &TranscriptionOptions {
                    use_gpu: false,
                    ..TranscriptionOptions::default()
                },
            )
            .expect_err("invalid native model");
        assert_eq!(error, AsrRuntimeError::ModelUnavailable);
        assert_eq!(runtime.clear_cache(), Ok(0));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_a_changed_descriptor_before_native_loading() {
        let root = std::env::temp_dir().join(format!("whisper-runtime-{}", OperationId::new()));
        let (repository, mut descriptor) = fixture(&root);
        descriptor.artifact_hash = ContentHash::parse("ab".repeat(32)).expect("different hash");
        let runtime = WhisperCppRuntime::new(repository);
        assert_eq!(
            runtime.preload(&descriptor, &TranscriptionOptions::default()),
            Err(AsrRuntimeError::ModelUnavailable)
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn cancellation_precedes_model_resolution() {
        let root = std::env::temp_dir().join(format!("whisper-runtime-{}", OperationId::new()));
        let (repository, descriptor) = fixture(&root);
        let runtime = WhisperCppRuntime::new(repository);
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert_eq!(
            runtime.transcribe(
                &descriptor,
                &[0.0; 160],
                "",
                &TranscriptionOptions::default(),
                &cancellation,
            ),
            Err(AsrRuntimeError::Cancelled)
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn native_abort_callback_observes_the_job_token() {
        let cancellation = CancellationToken::new();
        let pointer = std::ptr::from_ref(&cancellation).cast_mut().cast();
        assert!(!unsafe { whisper_should_abort(pointer) });
        cancellation.cancel();
        assert!(unsafe { whisper_should_abort(pointer) });
    }
}
