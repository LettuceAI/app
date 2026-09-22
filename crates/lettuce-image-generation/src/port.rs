use async_trait::async_trait;
use lettuce_conversations::InferenceUsage;
use lettuce_jobs::handle::CancellationToken;
use lettuce_models::{ProviderAccount, StableDiffusionLora, StableDiffusionSettings};
use lettuce_types::{JobId, ModelProfileId};

/// An image the provider receives, already read from the media store.
#[derive(Clone, PartialEq, Eq)]
pub struct ImageInput {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for ImageInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImageInput")
            .field("mime_type", &self.mime_type)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// Everything a provider or the local runtime needs for one generation. The
/// prompt is already composed and the LoRAs already merged.
#[derive(Debug, Clone)]
pub struct ProviderImageRequest {
    pub job_id: JobId,
    pub model_profile_id: ModelProfileId,
    pub account: ProviderAccount,
    pub external_model_id: String,
    pub model_display_name: String,
    pub prompt: String,
    pub settings: StableDiffusionSettings,
    pub loras: Vec<StableDiffusionLora>,
    pub input_images: Vec<ImageInput>,
    pub mask_image: Option<ImageInput>,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub style: Option<String>,
    pub count: u32,
    pub text_output: bool,
    pub cancellation: CancellationToken,
}

/// One image a provider returned. Remote URLs are fetched by the adapter so
/// every output reaches media ingestion as bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderImage {
    pub bytes: Vec<u8>,
    pub declared_mime_type: Option<String>,
    pub text: Option<String>,
}

impl std::fmt::Debug for ProviderImage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderImage")
            .field("bytes", &self.bytes.len())
            .field("declared_mime_type", &self.declared_mime_type)
            .field("text", &self.text.as_ref().map(String::len))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderImageOutput {
    pub images: Vec<ProviderImage>,
    pub usage: Option<InferenceUsage>,
}

/// Provider failures carry the message legacy showed the user.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageProviderError {
    #[error("{0}")]
    Failed(String),
    #[error("Provider {0} does not support image generation")]
    Unsupported(String),
    #[error("Local image generation was cancelled.")]
    Cancelled,
}

#[async_trait]
pub trait ImageProviderPort: Send + Sync {
    async fn generate(
        &self,
        request: ProviderImageRequest,
    ) -> Result<ProviderImageOutput, ImageProviderError>;
}
