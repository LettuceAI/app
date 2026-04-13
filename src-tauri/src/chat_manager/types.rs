use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::tooling::ToolCall;

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub enum PromptTemplateType {
    #[default]
    Undefined,
    DirectChat,
    GroupChatRoleplay,
    GroupChatConversational,
    DynamicMemorySummarizer,
    DynamicMemoryManager,
    ReplyHelperRoleplay,
    ReplyHelperConversational,
    AvatarGeneration,
    AvatarEditRequest,
    SceneGeneration,
    DesignReferenceWriter,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PromptEntryRole {
    System,
    User,
    Assistant,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PromptEntryPosition {
    Relative,
    InChat,
    Conditional,
    Interval,
}

impl Default for PromptEntryPosition {
    fn default() -> Self {
        PromptEntryPosition::Relative
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PromptEntryImageSlot {
    Character,
    Persona,
    ChatBackground,
    Avatar,
    References,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PromptEntryPayload {
    ImageSlot { slot: PromptEntryImageSlot },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PromptEntryChatMode {
    Direct,
    Group,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PromptEntryCondition {
    ChatMode {
        value: PromptEntryChatMode,
    },
    SceneGenerationEnabled {
        value: bool,
    },
    AvatarGenerationEnabled {
        value: bool,
    },
    HasScene {
        value: bool,
    },
    HasSceneDirection {
        value: bool,
    },
    HasPersona {
        value: bool,
    },
    MessageCountAtLeast {
        value: u32,
    },
    ParticipantCountAtLeast {
        value: u32,
    },
    KeywordAny {
        values: Vec<String>,
    },
    KeywordAll {
        values: Vec<String>,
    },
    KeywordNone {
        values: Vec<String>,
    },
    DynamicMemoryEnabled {
        value: bool,
    },
    HasMemorySummary {
        value: bool,
    },
    HasKeyMemories {
        value: bool,
    },
    HasLorebookContent {
        value: bool,
    },
    HasSubjectDescription {
        value: bool,
    },
    HasCurrentDescription {
        value: bool,
    },
    HasCharacterReferenceImages {
        value: bool,
    },
    HasChatBackground {
        value: bool,
    },
    HasPersonaReferenceImages {
        value: bool,
    },
    HasCharacterReferenceText {
        value: bool,
    },
    HasPersonaReferenceText {
        value: bool,
    },
    InputScopeAny {
        values: Vec<String>,
    },
    OutputScopeAny {
        values: Vec<String>,
    },
    ProviderIdAny {
        values: Vec<String>,
    },
    ReasoningEnabled {
        value: bool,
    },
    VisionEnabled {
        value: bool,
    },
    All {
        conditions: Vec<PromptEntryCondition>,
    },
    Any {
        conditions: Vec<PromptEntryCondition>,
    },
    Not {
        condition: Box<PromptEntryCondition>,
    },
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemPromptEntry {
    pub id: String,
    pub name: String,
    pub role: PromptEntryRole,
    pub content: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub injection_position: PromptEntryPosition,
    #[serde(default)]
    pub injection_depth: u32,
    #[serde(default)]
    pub conditional_min_messages: Option<u32>,
    #[serde(default)]
    pub interval_turns: Option<u32>,
    #[serde(default)]
    pub system_prompt: bool,
    #[serde(default)]
    pub conditions: Option<PromptEntryCondition>,
    #[serde(default)]
    pub prompt_entry_payload: Option<PromptEntryPayload>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemPromptTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub prompt_type: PromptTemplateType,
    pub content: String,
    #[serde(default)]
    pub entries: Vec<SystemPromptEntry>,
    #[serde(default)]
    pub condense_prompt_entries: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredential {
    pub id: String,
    pub provider_id: String,
    pub label: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub config: Option<Value>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    #[serde(default)]
    pub provider_credential_id: Option<String>,
    pub provider_label: String,
    pub display_name: String,
    pub created_at: u64,
    #[serde(default = "default_input_scopes")]
    pub input_scopes: Vec<String>,
    #[serde(default = "default_output_scopes")]
    pub output_scopes: Vec<String>,
    #[serde(default)]
    pub advanced_model_settings: Option<AdvancedModelSettings>,
    /// Reference to a system prompt template (if any)
    #[serde(default)]
    pub prompt_template_id: Option<String>,
    #[serde(default)]
    pub voice_config: Option<serde_json::Value>,
    /// DEPRECATED: Old system prompt field (migrated to templates)
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    pub system_prompt: Option<String>,
}

fn default_input_scopes() -> Vec<String> {
    vec!["text".to_string()]
}

fn default_output_scopes() -> Vec<String> {
    vec!["text".to_string()]
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub default_provider_credential_id: Option<String>,
    pub default_model_id: Option<String>,
    pub provider_credentials: Vec<ProviderCredential>,
    pub models: Vec<Model>,
    #[serde(default)]
    pub app_state: Value,
    #[serde(default)]
    pub advanced_model_settings: AdvancedModelSettings,
    #[serde(default)]
    pub advanced_settings: Option<AdvancedSettings>,
    /// Reference to app-wide system prompt template (if any)
    #[serde(default)]
    pub prompt_template_id: Option<String>,
    /// DEPRECATED: Old system prompt field (migrated to templates)
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    pub system_prompt: Option<String>,
    /// Migration version for data structure changes
    #[serde(default)]
    pub migration_version: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedSettings {
    #[serde(default)]
    pub summarisation_model_id: Option<String>,
    #[serde(default)]
    pub developer_mode_enabled: Option<bool>,
    #[serde(default)]
    pub dynamic_memory_structured_fallback_format: Option<DynamicMemoryStructuredFallbackFormat>,
    #[serde(default)]
    pub dynamic_memory_llama_sampler_overwrite_enabled: Option<bool>,
    #[serde(default)]
    pub avatar_generation_enabled: Option<bool>,
    #[serde(default)]
    pub avatar_generation_model_id: Option<String>,
    #[serde(default)]
    pub scene_generation_enabled: Option<bool>,
    #[serde(default)]
    pub scene_generation_mode: Option<String>,
    #[serde(default)]
    pub scene_generation_model_id: Option<String>,
    #[serde(default)]
    pub scene_writer_model_id: Option<String>,
    #[serde(default)]
    pub creation_helper_enabled: Option<bool>,
    #[serde(default)]
    pub creation_helper_model_id: Option<String>,
    #[serde(default)]
    pub help_me_reply_enabled: Option<bool>,
    #[serde(default)]
    pub help_me_reply_model_id: Option<String>,
    #[serde(default)]
    pub help_me_reply_streaming: Option<bool>,
    #[serde(default)]
    pub help_me_reply_max_tokens: Option<u32>,
    #[serde(default)]
    pub help_me_reply_style: Option<String>,
    #[serde(default)]
    pub dynamic_memory: Option<DynamicMemorySettings>,
    #[serde(default)]
    pub group_dynamic_memory: Option<DynamicMemorySettings>,
    #[serde(default)]
    pub manual_mode_context_window: Option<u32>,
    /// Max token capacity for embedding model (1024, 2048, or 4096)
    #[serde(default)]
    pub embedding_max_tokens: Option<u32>,
    #[serde(default)]
    pub host_api: Option<HostApiSettings>,
    #[serde(default)]
    pub accessibility: Option<AccessibilitySettings>,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DynamicMemoryStructuredFallbackFormat {
    Json,
    Xml,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HostApiSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_host_api_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_host_api_port")]
    pub port: u16,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub exposed_models: Vec<HostApiExposedModel>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HostApiExposedModel {
    pub id: String,
    pub model_id: String,
    #[serde(default = "default_host_api_exposed_model_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub label: Option<String>,
}

fn default_host_api_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_host_api_port() -> u16 {
    3333
}

fn default_host_api_exposed_model_enabled() -> bool {
    true
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilitySettings {
    pub send: AccessibilitySoundSettings,
    pub success: AccessibilitySoundSettings,
    pub failure: AccessibilitySoundSettings,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilitySoundSettings {
    pub enabled: bool,
    pub volume: f32,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum MemoryRetrievalStrategy {
    Smart,
    Cosine,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DynamicMemorySettings {
    pub enabled: bool,
    #[serde(default)]
    pub summary_message_interval: u32,
    #[serde(default)]
    pub max_entries: u32,
    #[serde(default = "default_min_similarity")]
    pub min_similarity_threshold: f32,
    #[serde(default = "default_retrieval_limit")]
    pub retrieval_limit: u32,
    #[serde(default = "default_retrieval_strategy")]
    pub retrieval_strategy: MemoryRetrievalStrategy,
    #[serde(default = "default_hot_memory_token_budget")]
    pub hot_memory_token_budget: u32,
    /// Score reduction per memory cycle (0.05-0.15 recommended)
    #[serde(default = "default_decay_rate")]
    pub decay_rate: f32,
    /// Score below which memories are demoted to cold (0.2-0.4 recommended)
    #[serde(default = "default_cold_threshold")]
    pub cold_threshold: f32,
    /// Default delete confidence when the model omits it. Lower values prefer soft-delete.
    #[serde(default = "default_delete_confidence")]
    pub delete_confidence_default: f32,
    /// Maximum fraction of the starting hot set that can be hard-deleted in one cycle.
    #[serde(default = "default_max_hard_delete_ratio_per_cycle")]
    pub max_hard_delete_ratio_per_cycle: f32,
    /// v2 exclusive: Use last 2 messages for better memory retrieval
    #[serde(default = "default_context_enrichment")]
    pub context_enrichment_enabled: bool,
    #[serde(default)]
    pub recursive_memory_loops: bool,
    #[serde(default = "default_recursive_memory_loop_hard_cap")]
    pub recursive_memory_loop_hard_cap: u32,
}

fn default_min_similarity() -> f32 {
    0.5 // Default threshold - memories below this score are excluded
}

fn default_hot_memory_token_budget() -> u32 {
    2048 // Default token budget for hot memories
}

fn default_retrieval_limit() -> u32 {
    5 // Default max memories retrieved per turn
}

fn default_retrieval_strategy() -> MemoryRetrievalStrategy {
    MemoryRetrievalStrategy::Smart
}

fn default_decay_rate() -> f32 {
    0.1 // Score reduction per memory cycle
}

fn default_cold_threshold() -> f32 {
    0.4 // Memories below this score are demoted to cold
}

fn default_delete_confidence() -> f32 {
    0.5 // Omitted confidence should prefer cold storage over hard delete
}

fn default_recursive_memory_loop_hard_cap() -> u32 {
    20
}

fn default_max_hard_delete_ratio_per_cycle() -> f32 {
    0.5 // At most half of the hot set can be hard-deleted per cycle
}

fn default_context_enrichment() -> bool {
    true // v2 exclusive: Use last 2 messages for better retrieval
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedModelSettings {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_output_tokens: Option<u32>,
    pub context_length: Option<u32>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub top_k: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_steps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_cfg_scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_sampler: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_seed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_negative_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_denoising_strength: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_size: Option<String>,
    pub llama_gpu_layers: Option<u32>,
    pub llama_threads: Option<u32>,
    pub llama_threads_batch: Option<u32>,
    pub llama_seed: Option<u32>,
    pub llama_rope_freq_base: Option<f64>,
    pub llama_rope_freq_scale: Option<f64>,
    pub llama_offload_kqv: Option<bool>,
    pub llama_batch_size: Option<u32>,
    pub llama_kv_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_flash_attention: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_chat_template_override: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_mmproj_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_chat_template_preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_raw_completion_fallback: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_strict_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_sampler_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_sampler_order: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_min_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_typical_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llama_last_runtime_report: Option<serde_json::Value>,
    pub ollama_num_ctx: Option<u32>,
    pub ollama_num_predict: Option<u32>,
    pub ollama_num_keep: Option<u32>,
    pub ollama_num_batch: Option<u32>,
    pub ollama_num_gpu: Option<u32>,
    pub ollama_num_thread: Option<u32>,
    pub ollama_tfs_z: Option<f64>,
    pub ollama_typical_p: Option<f64>,
    pub ollama_min_p: Option<f64>,
    pub ollama_mirostat: Option<u32>,
    pub ollama_mirostat_tau: Option<f64>,
    pub ollama_mirostat_eta: Option<f64>,
    pub ollama_repeat_penalty: Option<f64>,
    pub ollama_seed: Option<u32>,
    pub ollama_stop: Option<Vec<String>>,
    // Reasoning/thinking settings
    #[serde(default)]
    pub reasoning_enabled: Option<bool>,
    #[serde(default)]
    pub reasoning_effort: Option<String>, // "low", "medium", "high"
    #[serde(default)]
    pub reasoning_budget_tokens: Option<u32>,
    // Caching settings
    #[serde(default)]
    pub prompt_caching_enabled: Option<bool>,
    pub prompt_caching_ttl: Option<String>,
}

impl Default for AdvancedModelSettings {
    fn default() -> Self {
        Self {
            temperature: None,
            top_p: None,
            max_output_tokens: Some(2048),
            context_length: None,
            frequency_penalty: None,
            presence_penalty: None,
            top_k: None,
            sd_steps: None,
            sd_cfg_scale: None,
            sd_sampler: None,
            sd_seed: None,
            sd_negative_prompt: None,
            sd_denoising_strength: None,
            sd_size: None,
            llama_gpu_layers: None,
            llama_threads: None,
            llama_threads_batch: None,
            llama_seed: None,
            llama_rope_freq_base: None,
            llama_rope_freq_scale: None,
            llama_offload_kqv: None,
            llama_batch_size: None,
            llama_kv_type: None,
            llama_flash_attention: None,
            llama_chat_template_override: None,
            llama_mmproj_path: None,
            llama_chat_template_preset: None,
            llama_raw_completion_fallback: None,
            llama_strict_mode: None,
            llama_sampler_profile: None,
            llama_sampler_order: None,
            llama_min_p: None,
            llama_typical_p: None,
            llama_last_runtime_report: None,
            ollama_num_ctx: None,
            ollama_num_predict: None,
            ollama_num_keep: None,
            ollama_num_batch: None,
            ollama_num_gpu: None,
            ollama_num_thread: None,
            ollama_tfs_z: None,
            ollama_typical_p: None,
            ollama_min_p: None,
            ollama_mirostat: None,
            ollama_mirostat_tau: None,
            ollama_mirostat_eta: None,
            ollama_repeat_penalty: None,
            ollama_seed: None,
            ollama_stop: None,
            reasoning_enabled: None,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            prompt_caching_enabled: Some(false),
            prompt_caching_ttl: Some("5min".to_string()),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImageAttachment {
    pub id: String,
    pub data: String,
    pub mime_type: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub storage_path: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: u64,
    #[serde(default)]
    pub usage: Option<UsageSummary>,
    #[serde(default)]
    pub variants: Vec<MessageVariant>,
    #[serde(default)]
    pub selected_variant_id: Option<String>,
    #[serde(default)]
    pub memory_refs: Vec<String>,
    /// Lorebook entries used during this message generation.
    #[serde(default)]
    pub used_lorebook_entries: Vec<String>,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub attachments: Vec<ImageAttachment>,
    /// Reasoning/thinking content from thinking models (not sent in API requests)
    #[serde(default)]
    pub reasoning: Option<String>,
    /// Model used to generate this message (assistant messages)
    #[serde(default)]
    pub model_id: Option<String>,
    /// Primary model that failed before falling back for this message
    #[serde(default)]
    pub fallback_from_model_id: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MessageVariant {
    pub id: String,
    pub content: String,
    pub created_at: u64,
    #[serde(default)]
    pub usage: Option<UsageSummary>,
    #[serde(default)]
    pub attachments: Vec<ImageAttachment>,
    #[serde(default)]
    pub reasoning: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEmbedding {
    pub id: String,
    pub text: String,
    pub embedding: Vec<f32>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub token_count: u32,
    /// If true, this memory is in cold storage (not injected into context)
    #[serde(default)]
    pub is_cold: bool,
    /// Last time this memory was accessed/retrieved (for demotion scoring)
    #[serde(default)]
    pub last_accessed_at: u64,
    /// Importance score (0.0-1.0) - decays over time, memories below threshold go cold
    #[serde(default = "default_importance_score")]
    pub importance_score: f32,
    /// If true, this memory never decays (user/LLM marked as critical)
    #[serde(default)]
    pub is_pinned: bool,
    /// Number of times this memory was retrieved for context
    #[serde(default)]
    pub access_count: u32,
    /// Ephemeral match score (similarity) from retrieval
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_score: Option<f32>,
    /// Category tag for clustering (e.g. character_trait, relationship, plot_event)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

fn default_importance_score() -> f32 {
    1.0
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SceneVariant {
    pub id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    pub created_at: u64,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    pub created_at: u64,
    #[serde(default)]
    pub variants: Vec<SceneVariant>,
    #[serde(default)]
    pub selected_variant_id: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub character_id: String,
    pub title: String,
    #[serde(default)]
    pub background_image_path: Option<String>,
    /// DEPRECATED: System prompts are now always rebuilt dynamically
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub selected_scene_id: Option<String>,
    #[serde(default)]
    pub prompt_template_id: Option<String>,
    #[serde(default)]
    pub persona_id: Option<String>,
    #[serde(default)]
    pub persona_disabled: bool,
    #[serde(default)]
    pub voice_autoplay: Option<bool>,
    #[serde(default)]
    pub advanced_model_settings: Option<AdvancedModelSettings>,
    #[serde(default)]
    pub memories: Vec<String>,
    #[serde(default)]
    pub memory_embeddings: Vec<MemoryEmbedding>,
    #[serde(default)]
    pub memory_summary: Option<String>,
    #[serde(default)]
    pub memory_summary_token_count: u32,
    #[serde(default)]
    pub memory_tool_events: Vec<serde_json::Value>,
    #[serde(default)]
    pub memory_status: Option<String>,
    #[serde(default)]
    pub memory_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_progress_step: Option<u32>,
    #[serde(default)]
    pub messages: Vec<StoredMessage>,
    #[serde(default)]
    pub archived: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub avatar_path: Option<String>,
    #[serde(default)]
    pub design_description: Option<String>,
    #[serde(default)]
    pub design_reference_image_ids: Vec<String>,
    #[serde(default)]
    pub background_image_path: Option<String>,
    #[serde(default)]
    pub definition: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub default_scene_id: Option<String>,
    #[serde(default)]
    pub default_model_id: Option<String>,
    #[serde(default)]
    pub fallback_model_id: Option<String>,
    #[serde(default = "default_memory_type")]
    pub memory_type: String,
    /// Reference to a character-specific system prompt template (if any)
    #[serde(default)]
    pub prompt_template_id: Option<String>,
    #[serde(default)]
    pub group_chat_prompt_template_id: Option<String>,
    #[serde(default)]
    pub group_chat_roleplay_prompt_template_id: Option<String>,
    /// DEPRECATED: Old system prompt field (migrated to templates)
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    pub system_prompt: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

fn default_memory_type() -> String {
    "manual".to_string()
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Persona {
    pub id: String,
    pub title: String,
    pub description: String,
    pub nickname: Option<String>,
    #[serde(default)]
    pub avatar_path: Option<String>,
    #[serde(default)]
    pub design_description: Option<String>,
    #[serde(default)]
    pub design_reference_image_ids: Vec<String>,
    #[serde(default)]
    pub is_default: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    #[serde(default)]
    pub cached_prompt_tokens: Option<u64>,
    #[serde(default)]
    pub cache_write_tokens: Option<u64>,
    #[serde(default)]
    pub reasoning_tokens: Option<u64>,
    #[serde(default)]
    pub image_tokens: Option<u64>,
    #[serde(default)]
    pub web_search_requests: Option<u64>,
    #[serde(default)]
    pub api_cost: Option<f64>,
    #[serde(default)]
    pub response_id: Option<String>,
    #[serde(default)]
    pub first_token_ms: Option<u64>,
    #[serde(default)]
    pub tokens_per_second: Option<f64>,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurnResult {
    pub session_id: String,
    pub session_updated_at: u64,
    pub request_id: Option<String>,
    pub user_message: StoredMessage,
    pub assistant_message: StoredMessage,
    pub usage: Option<UsageSummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionArgs {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "characterId")]
    pub character_id: String,
    #[serde(alias = "userMessage")]
    pub user_message: String,
    #[serde(alias = "personaId")]
    pub persona_id: Option<String>,
    #[serde(default, alias = "swapPlaces")]
    pub swap_places: Option<bool>,
    pub stream: Option<bool>,
    #[serde(alias = "requestId")]
    pub request_id: Option<String>,
    #[serde(default)]
    pub attachments: Vec<ImageAttachment>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRegenerateArgs {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "messageId")]
    pub message_id: String,
    #[serde(default, alias = "swapPlaces")]
    pub swap_places: Option<bool>,
    pub stream: Option<bool>,
    #[serde(alias = "requestId")]
    pub request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatContinueArgs {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "characterId")]
    pub character_id: String,
    #[serde(alias = "personaId")]
    pub persona_id: Option<String>,
    #[serde(default, alias = "swapPlaces")]
    pub swap_places: Option<bool>,
    pub stream: Option<bool>,
    #[serde(alias = "requestId")]
    pub request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAddMessageAttachmentArgs {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "characterId")]
    pub character_id: String,
    #[serde(alias = "messageId")]
    pub message_id: String,
    /// "user" or "assistant"
    pub role: String,
    #[serde(alias = "attachmentId")]
    pub attachment_id: String,
    #[serde(alias = "base64Data")]
    pub base64_data: String,
    #[serde(alias = "mimeType")]
    pub mime_type: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGenerateSceneImageArgs {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "messageId")]
    pub message_id: String,
    #[serde(alias = "attachmentId")]
    pub attachment_id: String,
    #[serde(alias = "scenePrompt")]
    pub scene_prompt: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGenerateScenePromptArgs {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "messageId")]
    pub message_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGenerateDesignReferenceDescriptionArgs {
    #[serde(alias = "subjectName")]
    pub subject_name: Option<String>,
    #[serde(alias = "subjectDescription")]
    pub subject_description: Option<String>,
    #[serde(alias = "currentDescription")]
    pub current_description: Option<String>,
    #[serde(alias = "avatarImage")]
    pub avatar_image: Option<String>,
    #[serde(default, alias = "referenceImages")]
    pub reference_images: Vec<String>,
    #[serde(alias = "requestId")]
    pub request_id: Option<String>,
    #[serde(default)]
    pub stream: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateResult {
    pub session_id: String,
    pub session_updated_at: u64,
    pub request_id: Option<String>,
    pub assistant_message: StoredMessage,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueResult {
    pub session_id: String,
    pub session_updated_at: u64,
    pub request_id: Option<String>,
    pub assistant_message: StoredMessage,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEnvelope {
    #[serde(default)]
    pub code: Option<String>,
    pub message: String,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub retryable: Option<bool>,
    #[serde(default)]
    pub status: Option<u16>,
}

/// Provider-agnostic normalized stream/update events.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum NormalizedEvent {
    #[serde(rename = "delta")]
    Delta { text: String },
    #[serde(rename = "reasoning")]
    Reasoning { text: String },
    #[serde(rename = "usage")]
    Usage { usage: UsageSummary },
    #[serde(rename = "toolCall")]
    ToolCall { calls: Vec<ToolCall> },
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "error")]
    Error { envelope: ErrorEnvelope },
}

// Newtypes for stronger ids (not yet widely used – future-proofing)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
#[allow(dead_code)]
pub struct ProviderId(pub String);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
#[allow(dead_code)]
pub struct ModelId(pub String);

// Ergonomic conversions for constructing ProviderId
impl From<&str> for ProviderId {
    fn from(value: &str) -> Self {
        ProviderId(value.to_string())
    }
}

impl From<String> for ProviderId {
    fn from(value: String) -> Self {
        ProviderId(value)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MessageSearchResult {
    pub message_id: String,
    pub content: String,
    pub created_at: u64,
    pub role: String,
}
