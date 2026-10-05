use std::sync::{Arc, Mutex};

use lettuce_contracts::{ApiError, ApiErrorCode};
use lettuce_model_hub::{KokoroInstallStore, KokoroVoiceInstallStore};
use lettuce_platform::EspeakNgProcess;
use lettuce_jobs::handle::CancellationToken;
use lettuce_speech::{
    AsrModelDescriptor, AsrRuntime, AsrRuntimeError, RuntimeTranscription, TranscriptionOptions,
    TtsRuntime,
};

use crate::api::ApiContext;
use crate::api::error::api_error;
use crate::{MicrophoneCapture, OnnxRuntimePaths};

/// What the speech API needs from the host: the synthesis runtime over the
/// device's Kokoro files and ONNX Runtime, and the microphone.
/// `InstalledSpeech` is the production host; tests supply their own.
pub trait SpeechHost: Send + Sync {
    fn voice_creation_runtime(&self, context: &ApiContext) -> Result<Arc<dyn lettuce_speech::VoiceDesignRuntime>, ApiError> {
        let tls = context.backend().tls_policy().map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
        let client = lettuce_network::JsonClient::with_tls(&tls).map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
        Ok(Arc::new(lettuce_speech::ElevenLabsTtsRuntime::new(Arc::new(client))))
    }

    fn tts_runtime(&self, context: &ApiContext) -> Result<Arc<dyn TtsRuntime>, ApiError>;

    fn microphone(&self) -> Option<Arc<dyn MicrophoneCapture>>;

    /// The recognizer transcriptions run on: Whisper over the installed
    /// models unless a host brings its own.
    fn asr_runtime(&self, context: &ApiContext) -> Arc<dyn AsrRuntime> {
        Arc::new(BackendWhisper(context.clone()))
    }
}

/// The backend's Whisper runtime as an `AsrRuntime`.
struct BackendWhisper(ApiContext);

impl AsrRuntime for BackendWhisper {
    fn transcribe(
        &self,
        model: &AsrModelDescriptor,
        mono_16khz: &[f32],
        prompt: &str,
        options: &TranscriptionOptions,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeTranscription, AsrRuntimeError> {
        self.0
            .backend()
            .whisper_runtime()
            .transcribe(model, mono_16khz, prompt, options, cancellation)
    }
}

/// No synthesis runtime and no microphone.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSpeech;

impl SpeechHost for NoSpeech {
    fn tts_runtime(&self, _context: &ApiContext) -> Result<Arc<dyn TtsRuntime>, ApiError> {
        Err(api_error(
            ApiErrorCode::Unavailable,
            "no speech synthesis runtime is available",
        ))
    }

    fn microphone(&self) -> Option<Arc<dyn MicrophoneCapture>> {
        None
    }
}

/// The synthesis runtime over the app folder's Kokoro files, with the
/// eSpeak NG on the device's path, and the host's microphone.
pub struct InstalledSpeech {
    microphone: Option<Arc<dyn MicrophoneCapture>>,
    runtime: Mutex<Option<(std::path::PathBuf, Arc<dyn TtsRuntime>)>>,
}

impl InstalledSpeech {
    #[must_use]
    pub fn new(microphone: Option<Arc<dyn MicrophoneCapture>>) -> Self {
        Self {
            microphone,
            runtime: Mutex::new(None),
        }
    }
}

impl std::fmt::Debug for InstalledSpeech {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("InstalledSpeech").finish_non_exhaustive()
    }
}

impl SpeechHost for InstalledSpeech {
    fn tts_runtime(&self, context: &ApiContext) -> Result<Arc<dyn TtsRuntime>, ApiError> {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let folder = context.app_folder().ok_or_else(|| {
            api_error(ApiErrorCode::Unavailable, "no app data folder is open")
        })?;
        let unavailable =
            |error: &dyn std::fmt::Display| api_error(ApiErrorCode::Unavailable, error.to_string());
        let root = context.retained_model_roots()?.kokoro.map(std::path::PathBuf::from)
            .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "the Kokoro root is unavailable"))?;
        if let Some((installed_root, cached)) = runtime.as_ref()
            && installed_root == &root {
            return Ok(Arc::clone(cached));
        }
        std::fs::create_dir_all(&root).map_err(|error| unavailable(&error))?;
        let tls = context
            .backend()
            .tls_policy()
            .map_err(|error| unavailable(&error))?;
        let built: Arc<dyn TtsRuntime> = Arc::new(
            context
                .backend()
                .tts_runtime(
                    &tls,
                    KokoroInstallStore::open(&root).map_err(|error| unavailable(&error))?,
                    KokoroVoiceInstallStore::open(&root).map_err(|error| unavailable(&error))?,
                    Arc::new(EspeakNgProcess::from_path()),
                    OnnxRuntimePaths::legacy_layout(
                        folder,
                        context.resource_dir().map(std::path::PathBuf::from),
                    ),
                )
                .map_err(|error| unavailable(&error))?,
        );
        *runtime = Some((root, Arc::clone(&built)));
        Ok(built)
    }

    fn microphone(&self) -> Option<Arc<dyn MicrophoneCapture>> {
        self.microphone.clone()
    }
}
