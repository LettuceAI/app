use serde::{Deserialize, Serialize};

use crate::{GpuModeDto, RunnabilityLabel};

/// Chat models (GGUF files) or image models.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HfBrowseMode {
    #[default]
    Llm,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HfSort {
    TrendingScore,
    Downloads,
    Likes,
    LastModified,
}

/// A model search. `limit` defaults to 20 and is capped at 100.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfSearchRequest {
    pub query: String,
    pub limit: Option<u32>,
    pub sort: Option<HfSort>,
    pub offset: Option<u32>,
    pub author: Option<String>,
    pub mode: HfBrowseMode,
    /// Also lists repositories without GGUF files.
    pub unfiltered: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfModelSummary {
    pub model_id: String,
    pub author: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub likes: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub downloads: i64,
    pub tags: Vec<String>,
    pub pipeline_tag: Option<String>,
    pub last_modified: Option<String>,
    pub trending_score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfSearchResults {
    pub models: Vec<HfModelSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfModelRequest {
    pub model_id: String,
    pub mode: HfBrowseMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfModelFile {
    pub filename: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
    pub quantization: String,
    pub is_mmproj: bool,
    pub is_mtp: bool,
    pub imatrix: bool,
}

/// A repository's downloadable files, smallest first, and its GGUF summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfModelInfo {
    pub model_id: String,
    pub author: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub likes: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub downloads: i64,
    pub tags: Vec<String>,
    pub architecture: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub context_length: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub parameter_count: Option<u64>,
    pub files: Vec<HfModelFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfReadmeRequest {
    pub model_id: String,
}

/// The model card without its front matter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfReadme {
    pub markdown: String,
}

/// An author's GGUF models (`limit` defaults to 50, capped at 100) and
/// profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfAuthorRequest {
    pub author: String,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub sort: Option<HfSort>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfAuthorOverview {
    pub name: String,
    pub fullname: Option<String>,
    pub avatar_url: Option<String>,
    pub details: Option<String>,
    pub kind: Option<String>,
    pub is_pro: bool,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub num_models: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub num_datasets: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub num_spaces: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub num_likes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub num_followers: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub num_following: u64,
    pub created_at: Option<String>,
}

/// The author's profile, or why neither the user nor the organization
/// profile could be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum HfAuthorProfile {
    Found { overview: HfAuthorOverview },
    Unavailable { failure: Option<crate::HfFailure> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfAuthor {
    pub profile: HfAuthorProfile,
    pub models: Vec<HfModelSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfAvatarsRequest {
    pub authors: Vec<String>,
}

/// An author's avatar; `url` is `None` when neither lookup found one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfAvatar {
    pub author: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfAvatars {
    pub avatars: Vec<HfAvatar>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfRunnabilityFile {
    pub filename: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
}

/// Files of `model_id` judged against this machine, or against the machine
/// behind an Ollama account's Sprout probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfRunnabilityRequest {
    pub model_id: String,
    pub files: Vec<HfRunnabilityFile>,
    pub ollama_account_id: Option<String>,
}

/// Where the planner is asked to keep the KV cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HfKvPlacement {
    Auto,
    Ram,
    Vram,
}

/// The planner's current choice for one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfPlanChoice {
    pub filename: String,
    pub kv_type: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub context_length: u64,
    pub model_offload: HfModelOffload,
    pub kv_placement: HfKvPlacement,
}

/// A recommendation for the files of `model_id`; `sidecar_reserve_bytes`
/// (the projector and draft model chosen) and `plan` (the planner's
/// current choice) shape the planner's limits and report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfRecommendationRequest {
    pub model_id: String,
    pub files: Vec<HfRunnabilityFile>,
    pub ollama_account_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub sidecar_reserve_bytes: Option<u64>,
    pub plan: Option<HfPlanChoice>,
}

/// Where the planner expects the model and its KV cache to live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HfPlanGpuMode {
    Full,
    NearFull,
    KvSpill,
    KvHeavySpill,
    RamModelVramCtx,
    RamModelRamCtx,
    MostLayers,
    HalfLayers,
    FewLayers,
    Cpu,
    GpuUnavailable,
}

/// How much memory the planner's choice leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HfHeadroomStatus {
    Comfortable,
    Ok,
    Tight,
    Risky,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HfRunStatus {
    Yes,
    Borderline,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HfSpeed {
    Fast,
    Medium,
    Slow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfPlanKvDistribution {
    pub vram_percent: u32,
    pub on_vram_bytes: f64,
    pub on_ram_bytes: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfPlanUpgrade {
    pub filename: String,
    pub score: u32,
}

/// The planner's report for its choice: the context it allows and uses,
/// the memory it needs, its score, the GPU plan (offload share, layers, the
/// longest all-VRAM context when the chosen one spills, the KV split), the
/// GPU layer count a download stores, a better quantization and the context
/// the file switch suggests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfPlan {
    pub filename: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub max_context: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub context_length: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub effective_kv_context: u64,
    pub kv_bytes: f64,
    pub overhead_bytes: f64,
    pub total_needed_bytes: f64,
    pub gpu_resident_bytes: f64,
    pub headroom_bytes: f64,
    pub vram_budget_bytes: f64,
    pub score: u32,
    pub label: RunnabilityLabel,
    pub fits_vram: bool,
    pub gpu_mode: HfPlanGpuMode,
    pub gpu_score: f64,
    pub memory_score: u32,
    pub kv_score: u32,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub gpu_optimal_context: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub ram_max_context: u64,
    pub show_gpu_planning: bool,
    pub offload_percent: u32,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub total_layers: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub recommended_layers: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub full_gpu_context: Option<u64>,
    pub kv_distribution: Option<HfPlanKvDistribution>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub mixed_gpu_layers: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub requested_gpu_layers: Option<u64>,
    pub upgrade: Option<HfPlanUpgrade>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub default_context: u64,
    pub headroom: HfHeadroomStatus,
    pub run: HfRunStatus,
    pub prefill_speed: HfSpeed,
    pub generation_speed: HfSpeed,
    /// The KV offload a download with this choice stores.
    pub offload_kqv: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfRunnabilityScore {
    pub filename: String,
    pub score: u32,
    pub label: RunnabilityLabel,
    pub fits_in_ram: bool,
    pub fits_in_vram: bool,
    pub gpu_mode: GpuModeDto,
}

/// Scores per file. `hardware_available` is false for an Ollama account
/// without a Sprout probe (no scores then); `metadata_available` is false
/// when the GGUF header could not be read, so the KV cache is not counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfRunnability {
    pub hardware_available: bool,
    pub metadata_available: bool,
    pub scores: Vec<HfRunnabilityScore>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfModelArch {
    pub architecture: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub block_count: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub embedding_length: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub head_count: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub head_count_kv: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub context_length: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expert_count: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expert_used_count: Option<u64>,
    pub is_moe: bool,
    pub active_weight_ratio: Option<f64>,
    pub incomplete_parse: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfKvContextLimit {
    pub kv_type: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub max_context: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfFileRecommendation {
    pub filename: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
    pub quantization: String,
    pub quant_quality: u32,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub max_context_f16: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub max_context_q8_0: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub max_context_q4_0: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub optimal_gpu_ctx: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub optimal_ram_ctx: u64,
    /// The planner's longest context per KV type, next to the chosen
    /// sidecars.
    pub max_context_by_kv_type: Vec<HfKvContextLimit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfBestRecommendation {
    pub filename: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub context_length: u64,
    pub kv_type: String,
    pub score: u32,
    pub viable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfKvType {
    pub kv_type: String,
    pub bytes_per_value: f64,
}

/// The recommended file, context and KV type, with the limits the download
/// planner shows. `gpu_layer_count` is every block plus the output layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfRecommendation {
    pub hardware_available: bool,
    pub metadata_available: bool,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub available_ram: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub available_vram: u64,
    pub supports_gpu_offload: bool,
    pub unified_memory: bool,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub total_available: u64,
    pub kv_base_per_token: Option<f64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub kv_context_cap: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub model_max_context: u64,
    pub arch: Option<HfModelArch>,
    pub files: Vec<HfFileRecommendation>,
    pub best: Option<HfBestRecommendation>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub gpu_layer_count: Option<u64>,
    pub kv_types: Vec<HfKvType>,
    pub plan: Option<HfPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HfModelOffload {
    Auto,
    Cpu,
    Gpu,
    Mixed,
}

/// How a downloaded model is set up; with `create_model` it becomes a
/// llama.cpp model (an existing one with the same file is reused).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfDownloadSetup {
    pub display_name: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub context_length: Option<u64>,
    pub kv_type: Option<String>,
    pub offload_kqv: Option<bool>,
    pub gpu_layers: Option<u32>,
    pub model_offload: Option<HfModelOffload>,
    pub create_model: bool,
}

/// Downloads a GGUF file with its projector and MTP draft model as one
/// job, pinned to `revision` (the current one when absent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfDownloadRequest {
    pub repo: String,
    pub revision: Option<String>,
    pub file: String,
    pub mmproj_file: Option<String>,
    pub mtp_file: Option<String>,
    /// The model file carries its own MTP head.
    pub mtp_bundled: bool,
    pub setup: HfDownloadSetup,
    pub client_operation_id: String,
}

/// Whether the saved token works. `Unknown` means Hugging Face could not be
/// asked (`offline`) or answered with an error that is not a refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum HfTokenStatus {
    Missing,
    Valid { username: String },
    Invalid,
    Unknown { offline: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfAuthSaveRequest {
    pub token: String,
}
