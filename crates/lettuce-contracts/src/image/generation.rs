use serde::{Deserialize, Serialize};

use crate::AssetRef;

/// Why an image operation failed, for the UI to act on. The engine's own
/// words travel next to it as a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ImageFailureKind {
    Cancelled,
    LocalUnsupported,
    OutdatedRegistration,
    ModelFileMissing,
    ModelNotConfigured,
    RuntimeNotInstalled,
    RuntimeIncompatible,
    UpscalerMissing,
    InvalidRequest,
    ServerStartFailed,
    ServerNotReady,
    EngineRejected,
    EngineFailed,
    EngineTimedOut,
    OutOfMemory,
    LoraConflict,
    LoraInUse,
    LoraInvalid,
    StorageFailed,
    ProviderFailed,
    NoImageReturned,
    OutputRejected,
    ModelMissing,
    Interrupted,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageFailure {
    pub kind: ImageFailureKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ImagePhase {
    Starting,
    Loading,
    Sampling,
    Queued,
    Generating,
    Retrying,
    Cancelled,
}

/// What a running local generation reports, streamed with `job_watch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageProgress {
    pub phase: ImagePhase,
    pub step: Option<u32>,
    pub total_steps: Option<u32>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub queue_position: Option<u64>,
    pub preview_asset: Option<AssetRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageLora {
    pub path: String,
    pub multiplier: f64,
    #[serde(default)]
    pub is_high_noise: bool,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ImageCacheMode {
    Disabled,
    Easycache,
    Ucache,
    Dbcache,
    Taylorseer,
    CacheDit,
    Spectrum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ImageOffloadMode {
    Auto,
    Gpu,
    Mixed,
}

/// The sampling settings a request lays over its model's own; a field left
/// out keeps the model's value. `base_loras` replaces the model's base LoRAs
/// (an empty list drops them).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageSettings {
    #[serde(default)]
    pub steps: Option<u32>,
    #[serde(default)]
    pub cfg_scale: Option<f64>,
    #[serde(default)]
    pub sampler: Option<String>,
    #[serde(default)]
    pub scheduler: Option<String>,
    #[serde(default)]
    pub seed: Option<u32>,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default)]
    pub denoising_strength: Option<f64>,
    #[serde(default)]
    pub image_cfg_scale: Option<f64>,
    #[serde(default)]
    pub distilled_guidance: Option<f64>,
    #[serde(default)]
    pub eta: Option<f64>,
    #[serde(default)]
    pub flow_shift: Option<f64>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub vae_tiling_enabled: Option<bool>,
    #[serde(default)]
    pub vae_tile_size_x: Option<u32>,
    #[serde(default)]
    pub vae_tile_size_y: Option<u32>,
    #[serde(default)]
    pub vae_tile_overlap: Option<f64>,
    #[serde(default)]
    pub auto_resize_reference_images: Option<bool>,
    #[serde(default)]
    pub increase_reference_index: Option<bool>,
    #[serde(default)]
    pub hires_enabled: Option<bool>,
    #[serde(default)]
    pub hires_upscaler: Option<String>,
    #[serde(default)]
    pub hires_scale: Option<f64>,
    #[serde(default)]
    pub hires_width: Option<u32>,
    #[serde(default)]
    pub hires_height: Option<u32>,
    #[serde(default)]
    pub hires_steps: Option<u32>,
    #[serde(default)]
    pub hires_denoising_strength: Option<f64>,
    #[serde(default)]
    pub slg_scale: Option<f64>,
    #[serde(default)]
    pub slg_layers: Option<String>,
    #[serde(default)]
    pub slg_layer_start: Option<f64>,
    #[serde(default)]
    pub slg_layer_end: Option<f64>,
    #[serde(default)]
    pub cache_mode: Option<ImageCacheMode>,
    #[serde(default)]
    pub cache_option: Option<String>,
    #[serde(default)]
    pub offload_mode: Option<ImageOffloadMode>,
    #[serde(default)]
    pub extra_prompt: Option<String>,
    #[serde(default)]
    pub prompt_writer_instructions: Option<String>,
    #[serde(default)]
    pub base_loras: Option<Vec<ImageLora>>,
}

/// Which part of the app asks for the image; chat scene images go through
/// the message scene commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ImageRequestSource {
    Direct,
    Playground,
    CreationHelper,
}

/// Whether the images stay or are previews that expire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImageOutput {
    Retained,
    Preview {
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
        expires_at: i64,
    },
}

/// Where the image belongs for usage reporting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageAttribution {
    pub conversation_id: Option<String>,
    pub character_id: Option<String>,
}

/// One image generation. `request_id` is the idempotency key: repeating the
/// request returns its job, another request under the same id is `Conflict`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageGenerateRequest {
    pub request_id: String,
    pub model_id: String,
    pub prompt: String,
    #[serde(default)]
    pub settings: ImageSettings,
    #[serde(default)]
    pub input_images: Vec<String>,
    #[serde(default)]
    pub mask_image: Option<String>,
    #[serde(default)]
    pub loras: Vec<ImageLora>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
    pub source: ImageRequestSource,
    #[serde(default)]
    pub attribution: ImageAttribution,
    pub output: ImageOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GeneratedImage {
    pub asset: AssetRef,
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Text the provider returned with the image.
    pub text: Option<String>,
}

/// Where an upscale was asked from: an upscale of a playground entry's
/// image is recorded as a new entry that copies the model, prompt and seed
/// of `entry_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImageUpscaleOrigin {
    Playground { entry_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageUpscaleRequest {
    pub asset_id: String,
    pub origin: Option<ImageUpscaleOrigin>,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImageCapabilityTarget {
    Model {
        model_id: String,
    },
    Provider {
        provider_kind: String,
        model: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageCapabilitiesRequest {
    pub target: ImageCapabilityTarget,
}

/// What the playground form offers for a model or provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageCapabilities {
    pub provider_kind: String,
    pub local: bool,
    pub sizes: Vec<String>,
    pub default_size: Option<String>,
    pub samplers: Vec<String>,
    pub schedulers: Vec<String>,
    pub negative_prompt: bool,
    pub quality: Vec<String>,
    pub styles: Vec<String>,
    pub max_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PlaygroundHistoryListRequest {
    pub limit: Option<u32>,
    /// Lists entries created strictly before this time (unix ms).
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub before: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PlaygroundOrigin {
    Generated,
    Imported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PlaygroundStatus {
    Pending,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PlaygroundImage {
    /// The stored image; `None` for an imported image whose file was not
    /// carried over.
    pub asset: Option<AssetRef>,
    pub mime_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PlaygroundEntry {
    pub id: String,
    pub origin: PlaygroundOrigin,
    pub job_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    pub provider_kind: String,
    pub model_id: Option<String>,
    pub model_name: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub seed: Option<i64>,
    /// The generation parameters as JSON text.
    pub params_json: String,
    pub status: PlaygroundStatus,
    pub failure: Option<ImageFailure>,
    /// The entry whose image this one upscaled, when it is an upscale.
    pub upscale_of: Option<String>,
    pub images: Vec<PlaygroundImage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PlaygroundHistoryPage {
    pub entries: Vec<PlaygroundEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PlaygroundHistoryDeleteRequest {
    pub id: String,
    pub delete_images: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PlaygroundHistoryDeleted {
    /// The images removed with the entry.
    pub deleted_images: u32,
}
