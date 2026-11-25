use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lettuce_context::PromptVariable;
use lettuce_contracts::{ApiError, ApiErrorCode};
use lettuce_jobs::handle::CancellationToken;
use lettuce_model_hub::{KokoroInstallStore, KokoroVoiceInstallStore};
use lettuce_speech::{
    AsrModelDescriptor, AsrModelSource, AsrRuntime, AsrRuntimeError, CloudAsrProviderKind,
    LinkedProviderCredentials, ResolvedLinkedProvider, RuntimeTranscription, TranscriptionOptions,
    TtsRuntime, TtsRuntimeError,
};
use lettuce_types::ProviderAccountId;

use crate::api::ApiContext;
use crate::api::error::api_error;
use crate::generation::runtime_text::RuntimeText;
use crate::{BuiltInPromptId, MicrophoneCapture, OnnxRuntimePaths};

/// Shared app-layer resolver for linked cloud provider credentials and built-in
/// speech runtime prompts.
#[derive(Clone)]
pub(crate) struct AppLinkedProviderCredentials {
    context: ApiContext,
}

impl AppLinkedProviderCredentials {
    #[must_use]
    pub(crate) fn new(context: ApiContext) -> Self {
        Self { context }
    }

    pub(crate) fn speech_json_client(&self) -> Result<Arc<lettuce_network::JsonClient>, ApiError> {
        self.context
            .backend()
            .speech_json_client()
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))
    }

    pub(crate) async fn resolve_account(
        &self,
        account_id: ProviderAccountId,
    ) -> Result<(lettuce_models::ProviderAccount, lettuce_settings::SecretValue), TtsRuntimeError> {
        let account = lettuce_models::ProviderAccountRepository::get(
            self.context.backend().database(),
            account_id,
        )
        .map_err(|_| TtsRuntimeError::Unavailable {
            reason: "could not load linked provider account",
        })?
        .ok_or(TtsRuntimeError::Rejected {
            reason: "linked provider account was not found",
        })?;
        let key_ref = account.api_key_ref.ok_or(TtsRuntimeError::Unauthorized {
            reason: "linked provider account has no api key",
        })?;
        let secret = self
            .context
            .secret_store()
            .load(
                &key_ref,
                &lettuce_settings::SecretPurpose::ProviderApiKey {
                    owner: account.secret_owner_id,
                },
            )
            .await
            .map_err(|_| TtsRuntimeError::Unauthorized {
                reason: "linked provider api key is unavailable",
            })?;
        Ok((account, secret))
    }

    pub(crate) fn render_gemini_stt_instruction(
        &self,
        language: Option<&str>,
        hints: Option<&str>,
    ) -> Result<String, AsrRuntimeError> {
        let text = RuntimeText::load(
            self.context.backend().database(),
            BuiltInPromptId::SpeechRuntime,
        )
        .map_err(|_| AsrRuntimeError::Unavailable {
            reason: "speech runtime prompt catalog is unavailable",
        })?;
        let mut instruction = text
            .render_with("gemini_stt_instruction", [])
            .map_err(|_| AsrRuntimeError::Failed {
                reason: "gemini stt instruction prompt could not be rendered",
            })?;
        if let Some(lang) = language {
            let suffix = text
                .render_with(
                    "gemini_stt_language_suffix",
                    [(PromptVariable::SpeechLanguage, lang.to_owned())],
                )
                .map_err(|_| AsrRuntimeError::Failed {
                    reason: "gemini stt language suffix could not be rendered",
                })?;
            instruction.push_str(&suffix);
        }
        if let Some(hints) = hints {
            let suffix = text
                .render_with(
                    "gemini_stt_hints_suffix",
                    [(PromptVariable::SpeechHints, hints.to_owned())],
                )
                .map_err(|_| AsrRuntimeError::Failed {
                    reason: "gemini stt hints suffix could not be rendered",
                })?;
            instruction.push_str(&suffix);
        }
        Ok(instruction)
    }
}

#[async_trait]
impl LinkedProviderCredentials for AppLinkedProviderCredentials {
    async fn resolve_linked_provider(
        &self,
        account_id: ProviderAccountId,
    ) -> Result<ResolvedLinkedProvider, TtsRuntimeError> {
        let (account, api_key) = self.resolve_account(account_id).await?;
        Ok(ResolvedLinkedProvider {
            endpoint: account.endpoint,
            api_key,
        })
    }

    fn render_gemini_tts_text(
        &self,
        instructions: &str,
        speech_text: &str,
    ) -> Result<String, TtsRuntimeError> {
        let text = RuntimeText::load(
            self.context.backend().database(),
            BuiltInPromptId::SpeechRuntime,
        )
        .map_err(|_| TtsRuntimeError::Unavailable {
            reason: "speech runtime prompt catalog is unavailable",
        })?;
        text.render_with(
            "gemini_tts_directed_text",
            [
                (PromptVariable::SpeechInstructions, instructions.to_owned()),
                (PromptVariable::SpeechText, speech_text.to_owned()),
            ],
        )
        .map_err(|_| TtsRuntimeError::Failed {
            reason: "gemini tts directed prompt could not be rendered",
        })
    }
}

/// What the speech API needs from the host: the synthesis runtime over the
/// device's Kokoro files and ONNX Runtime, and the microphone.
/// `InstalledSpeech` is the production host; tests supply their own.
pub trait SpeechHost: Send + Sync {
    fn voice_creation_runtime(
        &self,
        context: &ApiContext,
    ) -> Result<Arc<dyn lettuce_speech::VoiceDesignRuntime>, ApiError> {
        let client = context
            .backend()
            .speech_json_client()
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
        Ok(Arc::new(lettuce_speech::ElevenLabsTtsRuntime::new(client)))
    }

    fn tts_runtime(&self, context: &ApiContext) -> Result<Arc<dyn TtsRuntime>, ApiError>;

    fn microphone(&self) -> Option<Arc<dyn MicrophoneCapture>>;

    /// The recognizer transcriptions run on: Whisper over the installed
    /// models unless a host brings its own.
    fn asr_runtime(&self, context: &ApiContext) -> Arc<dyn AsrRuntime> {
        Arc::new(BackendWhisper(context.clone()))
    }
}

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
        if let AsrModelSource::Cloud {
            provider_account_id,
            kind,
            model_id,
        } = model.source()
        {
            let resolver = AppLinkedProviderCredentials::new(self.0.clone());
            let client = self
                .0
                .backend()
                .speech_json_client()
                .map_err(|_| AsrRuntimeError::Unavailable {
                    reason: "http client is unavailable",
                })?;
            let language = options
                .language
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("auto"));
            let hints = {
                let trimmed = prompt.trim();
                (!trimmed.is_empty()).then_some(trimmed)
            };
            let gemini_instruction = if *kind == CloudAsrProviderKind::Gemini
                && !model_id.as_str().contains("transcribe")
            {
                Some(resolver.render_gemini_stt_instruction(language, hints)?)
            } else {
                None
            };
            let runtime = lettuce_speech::CloudAsrRuntime::new(client);
            let account_id = *provider_account_id;
            let kind = *kind;
            let model_id = model_id.as_str().to_owned();
            return tokio::runtime::Handle::current().block_on(async move {
                let resolved = resolver
                    .resolve_linked_provider(account_id)
                    .await
                    .map_err(|error| match error {
                        TtsRuntimeError::Unauthorized { reason } => {
                            AsrRuntimeError::Unauthorized { reason }
                        }
                        TtsRuntimeError::Rejected { reason } => {
                            AsrRuntimeError::ModelUnavailable { reason }
                        }
                        _ => AsrRuntimeError::Unavailable {
                            reason: "linked provider account is unavailable",
                        },
                    })?;
                runtime
                    .transcribe(
                        resolved.endpoint.as_deref(),
                        kind,
                        &model_id,
                        mono_16khz,
                        prompt,
                        gemini_instruction.as_deref(),
                        options,
                        &resolved.api_key,
                        cancellation,
                    )
                    .await
            });
        }
        self.0.backend().whisper_runtime().transcribe(
            model,
            mono_16khz,
            prompt,
            options,
            cancellation,
        )
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
        formatter
            .debug_struct("InstalledSpeech")
            .finish_non_exhaustive()
    }
}

impl SpeechHost for InstalledSpeech {
    fn tts_runtime(&self, context: &ApiContext) -> Result<Arc<dyn TtsRuntime>, ApiError> {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let folder = context
            .app_folder()
            .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no app data folder is open"))?;
        let unavailable =
            |error: &dyn std::fmt::Display| api_error(ApiErrorCode::Unavailable, error.to_string());
        let root = context
            .retained_model_roots()?
            .kokoro
            .map(std::path::PathBuf::from)
            .ok_or_else(|| {
                api_error(ApiErrorCode::Unavailable, "the Kokoro root is unavailable")
            })?;
        if let Some((installed_root, cached)) = runtime.as_ref()
            && installed_root == &root
        {
            return Ok(Arc::clone(cached));
        }
        std::fs::create_dir_all(&root).map_err(|error| unavailable(&error))?;
        let tls = context
            .backend()
            .tls_policy()
            .map_err(|error| unavailable(&error))?;
        let linked = Arc::new(AppLinkedProviderCredentials::new(context.clone()));
        let built: Arc<dyn TtsRuntime> = Arc::new(
            context
                .backend()
                .tts_runtime_with_cache(
                    &tls,
                    KokoroInstallStore::open(&root).map_err(|error| unavailable(&error))?,
                    KokoroVoiceInstallStore::open(&root).map_err(|error| unavailable(&error))?,
                    phonemizer(&root),
                    OnnxRuntimePaths::legacy_layout(
                        folder,
                        context.resource_dir().map(std::path::PathBuf::from),
                    ),
                    context.onnx_models().clone(),
                    Some(linked),
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

#[cfg(target_os = "android")]
fn phonemizer(root: &std::path::Path) -> Arc<dyn lettuce_platform::EspeakPhonemizer> {
    Arc::new(lettuce_platform::EspeakNgLibrary::new(
        root.join(lettuce_model_hub::ESPEAK_DATA_DIRECTORY),
    ))
}

#[cfg(not(target_os = "android"))]
fn phonemizer(_root: &std::path::Path) -> Arc<dyn lettuce_platform::EspeakPhonemizer> {
    Arc::new(lettuce_platform::EspeakNgProcess::from_path())
}
