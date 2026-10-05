use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioProviderConfiguration {
    Gemini {
        project_id: Option<String>,
        location: String,
    },
    Elevenlabs,
    FishTts,
    FishSpeech {
        base_url: Option<String>,
        request_path: Option<String>,
    },
    OpenAiCompatible {
        base_url: Option<String>,
        request_path: Option<String>,
    },
    Kokoro {
        variant: Option<String>,
    },
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AudioProviderDraft {
    pub configuration: AudioProviderConfiguration,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioProviderVerifyRequest {
    Saved { provider_id: String },
    Draft { draft: AudioProviderDraft },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AudioProviderRequest {
    pub provider_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AudioProviderCredentialStatus {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub generation: u64,
    pub available: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AudioProviderApiKeyRotateRequest {
    pub provider_id: String,
    pub api_key: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_generation: u64,
}

impl std::fmt::Debug for AudioProviderApiKeyRotateRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioProviderApiKeyRotateRequest")
            .field("provider_id", &self.provider_id)
            .field("api_key", &"[redacted]")
            .field("expected_generation", &self.expected_generation)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AudioProviderType {
    Gemini,
    Elevenlabs,
    FishTts,
    FishSpeech,
    OpenAiCompatible,
    Kokoro,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TtsModelsRequest {
    pub provider_type: AudioProviderType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TtsModelView {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AudioVoiceView {
    pub voice_id: String,
    pub name: String,
    pub preview_url: Option<String>,
    pub labels: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TtsCacheStats {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size_bytes: u64,
}

impl std::fmt::Debug for AudioProviderDraft {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioProviderDraft")
            .field("configuration", &self.configuration)
            .field("api_key", &self.api_key.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TtsSynthesizeRequest {
    pub request_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub voice_id: String,
    pub prompt: Option<String>,
    pub text: String,
    #[serde(default)]
    pub retained: bool,
}

/// Account metadata; credentials remain in the secret store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AudioProviderView {
    pub id: String,
    pub label: String,
    pub configuration: AudioProviderConfiguration,
    pub has_api_key: bool,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AudioProviderUpdateRequest {
    pub provider_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub label: String,
    pub configuration: AudioProviderConfiguration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AudioProviderDeleteRequest {
    pub provider_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UserVoiceView {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub model_id: String,
    pub voice_id: String,
    pub prompt: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UserVoiceRequest {
    pub voice_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UserVoiceUpdateRequest {
    pub id: String,
    pub provider_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub name: String,
    pub model_id: String,
    pub voice_id: String,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AudioProviderVoiceSearchRequest {
    pub provider_id: String,
    pub search: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct VoiceDesignPreviewRequest {
    pub provider_id: String,
    pub text_sample: String,
    pub voice_description: String,
    pub model_id: Option<String>,
    pub num_previews: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct VoiceDesignPreviewView {
    pub generated_voice_id: String,
    pub audio: crate::AssetRef,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MessageVoiceOverride {
    UserVoice {
        voice_id: String,
    },
    Provider {
        provider_id: String,
        voice_id: String,
        model_id: Option<String>,
        prompt: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageSpeakRequest {
    pub request_id: String,
    pub message_id: String,
    pub voice_override: Option<MessageVoiceOverride>,
    #[serde(default)]
    pub swap_places: bool,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AudioProviderCreateRequest {
    pub client_operation_id: String,
    pub label: String,
    pub draft: AudioProviderDraft,
}

impl std::fmt::Debug for AudioProviderCreateRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioProviderCreateRequest")
            .field("client_operation_id", &self.client_operation_id)
            .field("label", &self.label)
            .field("draft", &self.draft)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UserVoiceCreateRequest {
    pub client_operation_id: String,
    pub provider_id: String,
    pub name: String,
    pub model_id: String,
    pub voice_id: String,
    pub prompt: Option<String>,
}

/// Creates a provider voice from a selected design preview. Saving it to
/// the user's voice library is a separate operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct VoiceDesignCreateRequest {
    pub client_operation_id: String,
    pub provider_id: String,
    pub generated_voice_id: String,
    pub name: String,
    pub description: String,
}
