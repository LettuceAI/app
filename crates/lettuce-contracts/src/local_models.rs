use serde::{Deserialize, Serialize};

use crate::FileSource;

/// A GPU (or accelerator) llama.cpp can offload to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LlamaDevice {
    pub index: u32,
    pub name: String,
    pub description: String,
    pub backend: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub memory_total: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub memory_free: u64,
    pub device_type: String,
}

/// The devices llama.cpp can use; empty on mobile, where it does not run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LlamaDeviceList {
    pub devices: Vec<LlamaDevice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LlamaKvPlacement {
    Auto,
    Split,
    SystemRam,
    Pin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LlamaGpuDistribution {
    Balanced,
    Proportional,
    Priority,
    Manual,
}

/// Where a draft model (MTP) runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LlamaDraftPlacement {
    Auto,
    Gpu,
    Cpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LlamaGpuLayers {
    pub device_id: u32,
    pub layers: u32,
}

/// The model editor's unsaved llama.cpp load settings. KV types are
/// llama.cpp type names (`f16`, `q8_0`, ...).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LlamaSettingsDraft {
    pub offload_kqv: Option<bool>,
    pub kv_type: Option<String>,
    pub kv_type_k: Option<String>,
    pub kv_type_v: Option<String>,
    pub gpu_layers: Option<u32>,
    pub multi_gpu_enabled: Option<bool>,
    pub gpu_device_ids: Option<Vec<u32>>,
    pub gpu_distribution: Option<LlamaGpuDistribution>,
    pub gpu_manual_layers: Option<Vec<LlamaGpuLayers>>,
    pub single_gpu_device_id: Option<u32>,
    pub kv_placement: Option<LlamaKvPlacement>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub priority_vram_limit_bytes: Option<u64>,
    pub mmproj: Option<FileSource>,
    pub mtp_enabled: Option<bool>,
    pub mtp_placement: Option<LlamaDraftPlacement>,
    pub mtp_model: Option<FileSource>,
    pub dflash_enabled: Option<bool>,
    pub dflash_model: Option<FileSource>,
}

/// A model file with the editor's unsaved settings, or a saved llama.cpp
/// model with its stored ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LlamaModelTarget {
    Draft {
        model: FileSource,
        settings: Box<LlamaSettingsDraft>,
    },
    Saved {
        model_profile_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LlamaContextInfoRequest {
    pub target: LlamaModelTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LlamaDeviceMemory {
    pub index: u32,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub memory_free: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub memory_total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LlamaPlacement {
    pub total_gpu_layers: u32,
    pub per_device_layers: Vec<u32>,
}

/// How a model fits this machine with the given settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LlamaContextInfo {
    pub max_context_length: u32,
    pub recommended_context_length: Option<u32>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub available_memory_bytes: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub available_vram_bytes: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub model_size_bytes: Option<u64>,
    pub layer_count: Option<u32>,
    pub max_gpu_layers: Option<u32>,
    pub supports_gpu_offload: Option<bool>,
    pub selected_gpu_device_ids: Option<Vec<u32>>,
    pub per_device_vram: Option<Vec<LlamaDeviceMemory>>,
    pub estimated_placement: Option<LlamaPlacement>,
}

/// A model file, or a saved llama.cpp model's file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LlamaModelFile {
    File { model: FileSource },
    Saved { model_profile_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LlamaChatTemplateRequest {
    pub target: LlamaModelFile,
}

/// The chat template embedded in a GGUF file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LlamaChatTemplate {
    pub template: String,
}

/// Which path of a model points at a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LocalModelPathField {
    Model,
    Mmproj,
    Mtp,
    Dflash,
}

/// A saved model (or, without an id, the global model defaults) whose
/// paths point at a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalModelReference {
    pub model_profile_id: Option<String>,
    pub display_name: Option<String>,
    pub fields: Vec<LocalModelPathField>,
}

/// A GGUF file in the models folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalModelFile {
    /// The repository its folder is named after.
    pub repo: String,
    /// The path below the repository folder, `/`-separated.
    pub filename: String,
    pub path: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
    pub quantization: String,
    pub is_mmproj: bool,
    pub is_mtp: bool,
    pub is_dflash: bool,
    pub architecture: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub context_length: Option<u64>,
    pub used_by: Vec<LocalModelReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalModelList {
    pub files: Vec<LocalModelFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalModelDeleteRequest {
    pub path: String,
}

/// A deleted model file: whether llama.cpp was unloaded first because it
/// held the file, and the models still pointing at it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalModelDeleted {
    pub unloaded: bool,
    pub referencing_profiles: Vec<LocalModelReference>,
}

/// Moves a model file into the models folder, in a folder named after
/// `model_name` (else the file's name).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalModelAdoptRequest {
    pub source: FileSource,
    pub model_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalModelAdopted {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalModelsDir {
    pub path: String,
    pub default_path: String,
    pub is_custom: bool,
    pub model_count: u32,
}

/// Switches the models folder, moving what the current one holds and the
/// model paths into it when `move_existing` is set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalModelsDirSetRequest {
    pub path: String,
    pub move_existing: bool,
    pub client_operation_id: String,
}

/// The files loaded next to a model the editor has not saved.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalSidecarsDraft {
    pub mmproj: Option<FileSource>,
    pub mtp_enabled: bool,
    pub mtp_placement: Option<LlamaDraftPlacement>,
    pub mtp_model: Option<FileSource>,
    pub dflash_enabled: bool,
    pub dflash_model: Option<FileSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocalRunnabilityTarget {
    Draft {
        model: FileSource,
        sidecars: LocalSidecarsDraft,
    },
    Saved {
        model_profile_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalFileRunnabilityRequest {
    pub target: LocalRunnabilityTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RunnabilityLabel {
    Excellent,
    Good,
    Marginal,
    Poor,
    Unrunnable,
}

/// Where a model and its KV cache would live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum GpuModeDto {
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
}

/// How well a model file runs here. Without its GGUF header
/// (`metadata_available` false) the KV cache is not counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LocalFileRunnability {
    pub score: u32,
    pub label: RunnabilityLabel,
    pub fits_in_ram: bool,
    pub fits_in_vram: bool,
    pub memory_score: u32,
    pub gpu_score: u32,
    pub kv_score: u32,
    pub gpu_mode: GpuModeDto,
    pub quant_score: u32,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub available_ram: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub available_vram: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub model_size: u64,
    pub quantization: String,
    pub metadata_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RuntimeNoticeCode {
    MtpDisabledForVision,
    KvCacheMovedToRam,
}
