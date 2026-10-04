use serde::{Deserialize, Serialize};

use crate::{FileSource, ImageLora};

/// A file of an image model, by the part it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ImageComponentRole {
    DiffusionModel,
    TextEncoder,
    Vae,
    VisionEncoder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdRuntimeDependency {
    pub name: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub bytes: u64,
    pub sha256: Option<String>,
    pub download_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdRuntimeAsset {
    pub name: String,
    pub backend: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub bytes: u64,
    pub sha256: Option<String>,
    pub download_url: String,
    pub dependencies: Vec<SdRuntimeDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdRuntimeRelease {
    pub tag: String,
    pub name: String,
    pub published_at: Option<String>,
    pub prerelease: bool,
    pub assets: Vec<SdRuntimeAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdRuntimeReleases {
    pub releases: Vec<SdRuntimeRelease>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdCatalogVariant {
    pub id: String,
    pub label: String,
    pub description: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub download_bytes: u64,
    pub installed: bool,
    pub recommended: bool,
    pub smaller: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdCatalogProfile {
    pub id: String,
    pub display_name: String,
    pub family: String,
    pub description: String,
    pub license: String,
    pub source_url: String,
    pub supports_text_to_image: bool,
    pub supports_image_edit: bool,
    pub supports_lora: bool,
    pub max_reference_images: Option<u8>,
    pub requires_reference_image: bool,
    pub recommended_for_scenes: bool,
    pub default_width: u32,
    pub default_height: u32,
    pub default_steps: u16,
    pub default_cfg: f32,
    pub minimum_runtime_build: Option<u32>,
    pub variants: Vec<SdCatalogVariant>,
}

/// The model catalog with its install state and the engine builds on offer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdCatalog {
    pub runtime_supported: bool,
    pub unsupported_reason: Option<String>,
    pub runtime_releases: Vec<SdRuntimeRelease>,
    pub profiles: Vec<SdCatalogProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdRuntimeRef {
    pub release: String,
    pub asset: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdInstalledRuntime {
    pub release: String,
    pub asset: String,
    pub backend: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size_bytes: u64,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdRuntimeInventory {
    pub installed: Vec<SdInstalledRuntime>,
    pub active: Option<SdRuntimeRef>,
}

/// The variant to register once an engine build install has finished.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdVariantRef {
    pub profile_id: String,
    pub variant_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdRuntimeInstallRequest {
    pub release: String,
    pub asset: String,
    /// A catalog variant to register once the build and its files are in.
    pub then_register: Option<SdVariantRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdModelInstallRequest {
    pub profile_id: String,
    pub variant_id: String,
    pub release: String,
    pub asset: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdInstalledComponent {
    pub role: ImageComponentRole,
    pub filename: String,
    pub path: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub bytes_on_disk: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdInstalledModel {
    pub profile_id: String,
    pub variant_id: String,
    pub display_name: String,
    pub runtime_release: Option<String>,
    pub runtime_asset: Option<String>,
    pub runtime_backend: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub component_bytes_on_disk: u64,
    pub model_id: Option<String>,
    pub supports_text_to_image: bool,
    pub supports_image_edit: bool,
    pub recommended_for_scenes: bool,
    pub requires_reference_image: bool,
    pub default_width: u32,
    pub default_height: u32,
    pub default_steps: u16,
    pub default_cfg: f32,
    pub model_path: String,
    pub components: Vec<SdInstalledComponent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdInstalledModels {
    pub models: Vec<SdInstalledModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdModelUninstallRequest {
    pub profile_id: String,
    pub variant_id: String,
    pub also_remove_engine_if_unused: bool,
}

/// The model is removed even when a file cannot be; `left_behind` lists
/// those files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdUninstallOutcome {
    pub left_behind: Vec<String>,
}

/// The job that downloads the model's files, or `model_id` when every file
/// was already on disk and the model was registered at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdModelInstallStarted {
    pub job_id: Option<String>,
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdModelRepairRequest {
    pub profile_id: String,
    pub variant_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdModelRepaired {
    pub model_id: String,
}

/// A catalog fit test on an engine build; unset fields take the profile's
/// defaults. Installed variants are tested by running the engine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdRunnabilityRequest {
    pub profile_id: String,
    pub variant_id: String,
    pub runtime_release: String,
    pub runtime_asset: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub reference_image_count: Option<u8>,
    pub loras: Vec<ImageLora>,
    pub prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub sample_steps: Option<u32>,
    pub cfg_scale: Option<f64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub seed: Option<i64>,
    pub sample_method: Option<String>,
    pub batch_count: Option<u32>,
    /// Runs every sample step instead of a one-step probe.
    pub full_execution: bool,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdBundleRunnabilityRequest {
    pub profile_id: String,
    pub runtime_release: String,
    pub runtime_asset: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub diffusion_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub text_encoder_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub vae_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub vision_encoder_bytes: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RunnabilityStatus {
    IncompatibleRuntime,
    NotInstalled,
    EstimatedRunnable,
    CpuFallback,
    Inconclusive,
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdFitDevice {
    pub id: u32,
    pub name: String,
    pub description: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub total_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub free_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub budget_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SdFitComponent {
    Dit,
    Vae,
    Conditioner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SdPlanMode {
    DefaultBackend,
    Concurrent,
    TimeShare,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdFitPlacement {
    pub component: SdFitComponent,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub params_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub compute_reserve_bytes: u64,
    pub targets: Vec<String>,
    pub cpu: bool,
    pub split: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdFitEstimate {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub model_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub available_ram_bytes: Option<u64>,
    pub plan_mode: SdPlanMode,
    pub devices: Vec<SdFitDevice>,
    pub placements: Vec<SdFitPlacement>,
}

/// A runnability verdict: how it was reached, how exact it is and why.
/// `method`, `scope` and `placement_policy` are codes for the UI to localize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdRunnability {
    pub status: RunnabilityStatus,
    pub method: String,
    pub exact: bool,
    pub scope: String,
    pub placement_policy: String,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub elapsed_ms: Option<u64>,
    pub reason: String,
    pub estimate: Option<SdFitEstimate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdDeviceBudget {
    pub device_id: u32,
    pub gib: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdComputePolicy {
    pub multi_gpu_enabled: bool,
    pub gpu_device_ids: Vec<u32>,
    pub single_gpu_device_id: Option<u32>,
    pub device_budgets_gib: Vec<SdDeviceBudget>,
    /// `layer` or `row`.
    pub split_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdComputePolicyInfo {
    pub runtime_release: String,
    pub runtime_asset: String,
    pub backend: String,
    pub supports_row_split: bool,
    pub policy: SdComputePolicy,
    pub devices: Vec<SdFitDevice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdComputePolicySaveRequest {
    pub release: String,
    pub asset: String,
    pub policy: SdComputePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdDetectModelFileRequest {
    pub source: FileSource,
}

/// The architecture a catalog profile describes for assembling a model from
/// separate files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageBundleProfile {
    pub id: String,
    pub display_name: String,
    pub family: String,
    pub description: String,
    pub minimum_runtime_build: Option<u32>,
    pub required_roles: Vec<ImageComponentRole>,
    pub diffusion_markers: Vec<String>,
    pub encoder_markers: Vec<String>,
    pub encoder_parameter_billions: f32,
    pub recommended_repositories: Vec<RecommendedRepository>,
    pub supports_text_to_image: bool,
    pub supports_image_edit: bool,
    pub max_reference_images: Option<u8>,
    pub requires_reference_image: bool,
    pub recommended_for_scenes: bool,
    pub default_width: u32,
    pub default_height: u32,
    pub default_steps: u16,
    pub default_cfg: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct RecommendedRepository {
    pub role: ImageComponentRole,
    pub repository: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdDetectedModelFile {
    pub exists: bool,
    pub profile: Option<ImageBundleProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdDiskUsage {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub components_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub runtimes_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub loras_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub total_bytes: u64,
    pub has_engine: bool,
    pub engine_release: Option<String>,
    pub engine_backend: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ComponentSource {
    ImageComponents,
    LlmLibrary,
    ImageDownloads,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdComponentFile {
    pub path: String,
    pub filename: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub bytes: u64,
    pub role: Option<ImageComponentRole>,
    pub source: ComponentSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdComponentLibrary {
    pub files: Vec<SdComponentFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdUpscalerInventory {
    pub models: Vec<String>,
    /// File stems, the names the hires fix accepts.
    pub hires_upscaler_names: Vec<String>,
    pub recommended_filename: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub recommended_bytes: u64,
    pub recommended_installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SdUpscalerRemoveRequest {
    pub filename: String,
}

/// An image model file on disk below the image models folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DownloadedImageModel {
    pub model_id: String,
    pub filename: String,
    pub path: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
    pub quantization: String,
    pub is_mmproj: bool,
    pub architecture: Option<String>,
    pub role: ImageComponentRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DownloadedImageModels {
    pub models: Vec<DownloadedImageModel>,
}
