use crate::chat_manager::types::{MemoryEmbedding, MemoryEntityAnchor};
use serde::{Deserialize, Serialize};

fn default_speaker_selection_method() -> String {
    "llm".to_string()
}

fn default_memory_type() -> String {
    "manual".to_string()
}

fn default_character_mode() -> String {
    "roleplay".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub id: i64,
    pub default_provider_credential_id: Option<String>,
    pub default_model_id: Option<String>,
    pub app_state: String,
    pub advanced_model_settings: Option<String>,
    pub prompt_template_id: Option<String>,
    pub system_prompt: Option<String>,
    pub advanced_settings: Option<String>,
    pub migration_version: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Persona {
    pub id: String,
    pub title: String,
    pub description: String,
    pub nickname: Option<String>,
    pub avatar_path: Option<String>,
    pub avatar_crop_x: Option<f64>,
    pub avatar_crop_y: Option<f64>,
    pub avatar_crop_scale: Option<f64>,
    #[serde(default)]
    pub design_description: Option<String>,
    #[serde(default)]
    pub design_reference_image_ids: Option<String>,
    #[serde(default)]
    pub lora_name: Option<String>,
    #[serde(default)]
    pub lora_strength: Option<f64>,
    #[serde(default)]
    pub active_lorebook_ids: Option<String>,
    pub is_default: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    #[serde(default)]
    pub provider_credential_id: Option<String>,
    pub provider_label: String,
    pub display_name: String,
    pub created_at: i64,
    pub model_type: String,
    pub input_scopes: Option<String>,
    pub output_scopes: Option<String>,
    pub advanced_model_settings: Option<String>,
    pub prompt_template_id: Option<String>,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Secret {
    pub service: String,
    pub account: String,
    pub value: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderCredential {
    pub id: String,
    pub provider_id: String,
    pub label: String,
    pub api_key_ref: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub headers: Option<String>,
    pub config: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub prompt_type: String,
    pub content: String,
    pub entries: String,
    #[serde(default)]
    pub condense_prompt_entries: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelPricingCache {
    pub model_id: String,
    pub pricing_json: Option<String>,
    pub cached_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreationHelperSession {
    pub id: String,
    pub creation_goal: String,
    pub status: String,
    pub session_json: String,
    pub uploaded_images_json: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncCompanionScheduledNote {
    pub id: String,
    pub character_id: String,
    pub label: String,
    pub content: String,
    pub available_at: i64,
    pub expires_at: Option<i64>,
    pub recurrence: String,
    pub recurrence_window_ms: Option<i64>,
    pub enabled: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncCompanionTurnEffect {
    pub id: String,
    pub session_id: String,
    pub user_message_id: Option<String>,
    pub assistant_message_id: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: String,
    pub summary: Option<String>,
    pub relationship_delta: String,
    pub emotion_delta: String,
    pub signal_changes: String,
    pub memory_changes: String,
    pub source_window: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AudioProvider {
    pub id: String,
    pub provider_type: String,
    pub label: String,
    pub api_key: Option<String>,
    pub project_id: Option<String>,
    pub location: Option<String>,
    pub base_url: Option<String>,
    pub request_path: Option<String>,
    #[serde(default)]
    pub kokoro_variant: Option<String>,
    #[serde(default)]
    pub asset_root: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncAsrVocabularyTerm {
    pub term: String,
    pub normalized_term: String,
    pub language: Option<String>,
    pub category: Option<String>,
    pub scope: String,
    pub priority: i64,
    pub use_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncAsrCorrection {
    pub wrong: String,
    pub normalized_wrong: String,
    pub correct: String,
    pub normalized_correct: String,
    pub language: Option<String>,
    pub scope: String,
    pub confidence: f64,
    pub use_count: i64,
    pub accepted_count: i64,
    pub rejected_count: i64,
    pub seen_count: i64,
    pub last_seen_at: Option<String>,
    pub user_approved: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncAsrIgnoredSuggestion {
    pub wrong: String,
    pub normalized_wrong: String,
    pub correct: String,
    pub normalized_correct: String,
    pub language: Option<String>,
    pub scope: String,
    pub ignored_count: i64,
    pub last_ignored_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AudioVoiceCache {
    pub id: String,
    pub provider_id: String,
    pub voice_id: String,
    pub name: String,
    pub preview_url: Option<String>,
    pub labels: Option<String>,
    pub cached_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserVoice {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub model_id: String,
    pub voice_id: String,
    pub prompt: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupCharacter {
    pub id: String,
    pub name: String,
    pub character_ids: String,
    pub muted_character_ids: String,
    pub persona_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived: i64,
    pub chat_type: String,
    pub starting_scene: Option<String>,
    pub background_image_path: Option<String>,
    #[serde(default)]
    pub lorebook_ids: String,
    #[serde(default)]
    pub disable_character_lorebooks: i64,
    #[serde(default)]
    pub chat_appearance: Option<String>,
    #[serde(default = "default_speaker_selection_method")]
    pub speaker_selection_method: String,
    #[serde(default = "default_memory_type")]
    pub memory_type: String,
    #[serde(default = "default_character_model_overrides_json")]
    pub character_model_overrides: String,
    #[serde(default)]
    pub group_chat_prompt_template_id: Option<String>,
    #[serde(default)]
    pub group_chat_roleplay_prompt_template_id: Option<String>,
}

fn default_group_config_overrides_json() -> String {
    "{\"version\":1}".to_string()
}

fn default_character_model_overrides_json() -> String {
    "{}".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncLorebook {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub keyword_detection_mode: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncLorebookEntry {
    pub id: String,
    pub lorebook_id: String,
    pub title: String,
    pub enabled: i64,
    pub always_active: i64,
    pub keywords: String,
    pub case_sensitive: i64,
    #[serde(default = "default_keyword_match_mode")]
    pub keyword_match_mode: String,
    pub content: String,
    pub priority: i32,
    pub display_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

fn default_keyword_match_mode() -> String {
    "literal".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Character {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub avatar_crop_x: Option<f64>,
    pub avatar_crop_y: Option<f64>,
    pub avatar_crop_scale: Option<f64>,
    #[serde(default)]
    pub banner_crop_x: Option<f64>,
    #[serde(default)]
    pub banner_crop_y: Option<f64>,
    #[serde(default)]
    pub banner_crop_scale: Option<f64>,
    #[serde(default)]
    pub card_type: Option<String>,
    #[serde(default)]
    pub design_description: Option<String>,
    #[serde(default)]
    pub design_reference_image_ids: Option<String>,
    #[serde(default)]
    pub lora_name: Option<String>,
    #[serde(default)]
    pub lora_strength: Option<f64>,
    pub background_image_path: Option<String>,
    pub definition: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub scenario: Option<String>,
    #[serde(default)]
    pub creator_notes: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creator_notes_multilingual: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub tags: Option<String>,
    pub default_scene_id: Option<String>,
    pub default_model_id: Option<String>,
    #[serde(default = "default_character_mode")]
    pub mode: String,
    #[serde(default)]
    pub companion: Option<String>,
    pub memory_type: String,
    #[serde(default)]
    pub active_lorebook_ids: Option<String>,
    pub prompt_template_id: Option<String>,
    #[serde(default)]
    pub group_chat_prompt_template_id: Option<String>,
    #[serde(default)]
    pub group_chat_roleplay_prompt_template_id: Option<String>,
    pub system_prompt: Option<String>,
    pub voice_config: Option<String>,
    #[serde(default)]
    pub voice_autoplay: i64,
    pub disable_avatar_gradient: i64,
    #[serde(default)]
    pub avatar_gradient_source: Option<String>,
    pub custom_gradient_enabled: Option<i64>,
    pub custom_gradient_colors: Option<String>,
    pub custom_text_color: Option<String>,
    pub custom_text_secondary: Option<String>,
    #[serde(default)]
    pub chat_appearance: Option<String>,
    #[serde(default)]
    pub default_chat_template_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CharacterRule {
    pub id: Option<i64>,
    pub character_id: String,
    pub idx: i64,
    pub rule: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Scene {
    pub id: String,
    pub character_id: String,
    pub content: String,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub background_image_path: Option<String>,
    pub created_at: i64,
    pub selected_variant_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SceneVariant {
    pub id: String,
    pub scene_id: String,
    pub content: String,
    #[serde(default)]
    pub direction: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatTemplate {
    pub id: String,
    pub character_id: String,
    pub name: String,
    pub scene_id: Option<String>,
    pub prompt_template_id: Option<String>,
    #[serde(default)]
    pub lorebook_ids_override: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatTemplateMessage {
    pub id: String,
    pub template_id: String,
    pub idx: i64,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub character_id: String,
    pub title: String,
    #[serde(default)]
    pub parent_session_id: Option<String>,
    #[serde(default)]
    pub branched_from_message_id: Option<String>,
    #[serde(default)]
    pub root_session_id: Option<String>,
    #[serde(default)]
    pub background_image_path: Option<String>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub mode: String,
    pub selected_scene_id: Option<String>,
    #[serde(default)]
    pub prompt_template_id: Option<String>,
    #[serde(default)]
    pub lorebook_ids_override: Option<String>,
    #[serde(default)]
    pub author_note: Option<String>,
    pub persona_id: Option<String>,
    pub persona_disabled: Option<i64>,
    #[serde(default)]
    pub voice_autoplay: Option<i64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_output_tokens: Option<i64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub top_k: Option<i64>,
    #[serde(default)]
    pub advanced_model_settings: Option<String>,
    #[serde(default)]
    pub companion_state: Option<String>,
    pub memories: String,
    pub memory_embeddings: String,
    pub memory_summary: Option<String>,
    pub memory_summary_token_count: i64,
    pub memory_tool_events: String,
    pub archived: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub memory_status: Option<String>,
    pub memory_error: Option<String>,
    #[serde(default)]
    pub memory_progress_step: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompanionSharedMemory {
    pub character_id: String,
    pub memories: String,
    pub memory_embeddings: String,
    pub memory_summary: Option<String>,
    pub memory_summary_token_count: i64,
    pub memory_tool_events: String,
    #[serde(default)]
    pub memory_status: Option<String>,
    #[serde(default)]
    pub memory_error: Option<String>,
    #[serde(default)]
    pub memory_progress_step: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMemoryEmbeddingRecord {
    pub id: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub created_at: u64,
    pub token_count: u32,
    pub is_cold: bool,
    pub last_accessed_at: u64,
    pub importance_score: f32,
    pub persistence_importance: f32,
    pub prompt_importance: f32,
    pub volatility: f32,
    pub is_pinned: bool,
    pub access_count: u32,
    pub embedding_source_version: Option<String>,
    pub embedding_dimensions: Option<usize>,
    pub match_score: Option<f32>,
    pub category: Option<String>,
    pub observed_at: Option<u64>,
    pub observed_time_precision: Option<String>,
    pub canonical_entities: Vec<MemoryEntityAnchor>,
    pub fact_signature: Option<String>,
    pub fact_polarity: Option<i8>,
    pub source_role: Option<String>,
    pub source_message_id: Option<String>,
    pub superseded_by: Option<String>,
    pub superseded_at: Option<u64>,
    pub supersedes: Vec<String>,
}

impl From<MemoryEmbedding> for SyncMemoryEmbeddingRecord {
    fn from(memory: MemoryEmbedding) -> Self {
        Self {
            id: memory.id,
            text: memory.text,
            embedding: memory.embedding,
            created_at: memory.created_at,
            token_count: memory.token_count,
            is_cold: memory.is_cold,
            last_accessed_at: memory.last_accessed_at,
            importance_score: memory.importance_score,
            persistence_importance: memory.persistence_importance,
            prompt_importance: memory.prompt_importance,
            volatility: memory.volatility,
            is_pinned: memory.is_pinned,
            access_count: memory.access_count,
            embedding_source_version: memory.embedding_source_version,
            embedding_dimensions: memory.embedding_dimensions,
            match_score: memory.match_score,
            category: memory.category,
            observed_at: memory.observed_at,
            observed_time_precision: memory.observed_time_precision,
            canonical_entities: memory.canonical_entities,
            fact_signature: memory.fact_signature,
            fact_polarity: memory.fact_polarity,
            source_role: memory.source_role,
            source_message_id: memory.source_message_id,
            superseded_by: memory.superseded_by,
            superseded_at: memory.superseded_at,
            supersedes: memory.supersedes,
        }
    }
}

impl From<SyncMemoryEmbeddingRecord> for MemoryEmbedding {
    fn from(record: SyncMemoryEmbeddingRecord) -> Self {
        Self {
            id: record.id,
            text: record.text,
            embedding: record.embedding,
            created_at: record.created_at,
            token_count: record.token_count,
            is_cold: record.is_cold,
            last_accessed_at: record.last_accessed_at,
            importance_score: record.importance_score,
            persistence_importance: record.persistence_importance,
            prompt_importance: record.prompt_importance,
            volatility: record.volatility,
            is_pinned: record.is_pinned,
            access_count: record.access_count,
            embedding_source_version: record.embedding_source_version,
            embedding_dimensions: record.embedding_dimensions,
            match_score: record.match_score,
            category: record.category,
            observed_at: record.observed_at,
            observed_time_precision: record.observed_time_precision,
            canonical_entities: record.canonical_entities,
            fact_signature: record.fact_signature,
            fact_polarity: record.fact_polarity,
            source_role: record.source_role,
            source_message_id: record.source_message_id,
            superseded_by: record.superseded_by,
            superseded_at: record.superseded_at,
            supersedes: record.supersedes,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncedMemoryEmbedding {
    pub session_id: String,
    pub session_kind: String,
    pub memory: SyncMemoryEmbeddingRecord,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    #[serde(default)]
    pub visible_in_chat: i64,
    #[serde(default)]
    pub scene_edited: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    #[serde(default)]
    pub first_token_ms: Option<i64>,
    #[serde(default)]
    pub tokens_per_second: Option<f64>,
    #[serde(default)]
    pub mtp_stats: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    pub selected_variant_id: Option<String>,
    pub is_pinned: i64,
    pub memory_refs: String,
    #[serde(default)]
    pub used_lorebook_entries: String,
    pub attachments: String,
    pub reasoning: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageVariant {
    pub id: String,
    pub message_id: String,
    pub content: String,
    pub created_at: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    #[serde(default)]
    pub first_token_ms: Option<i64>,
    #[serde(default)]
    pub tokens_per_second: Option<f64>,
    #[serde(default)]
    pub mtp_stats: Option<String>,
    pub reasoning: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsageRecord {
    pub id: String,
    pub timestamp: i64,
    pub session_id: String,
    pub character_id: String,
    pub character_name: String,
    pub model_id: String,
    pub model_name: String,
    pub provider_id: String,
    pub provider_label: String,
    pub operation_type: Option<String>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub memory_tokens: Option<i64>,
    pub summary_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub image_tokens: Option<i64>,
    #[serde(default)]
    pub audio_tokens: Option<i64>,
    pub prompt_cost: Option<f64>,
    pub completion_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub success: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsageMetadata {
    pub usage_id: String,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupSession {
    pub id: String,
    #[serde(default)]
    pub group_character_id: Option<String>,
    pub name: String,
    pub character_ids: String,
    pub muted_character_ids: String,
    pub persona_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived: i64,
    pub chat_type: String,
    pub starting_scene: Option<String>,
    pub background_image_path: Option<String>,
    #[serde(default)]
    pub author_note: Option<String>,
    #[serde(default)]
    pub lorebook_ids: String,
    #[serde(default)]
    pub disable_character_lorebooks: i64,
    pub memories: String,
    pub memory_embeddings: String,
    pub memory_summary: String,
    pub memory_summary_token_count: i64,
    pub memory_tool_events: String,
    #[serde(default)]
    pub memory_status: Option<String>,
    #[serde(default)]
    pub memory_error: Option<String>,
    #[serde(default)]
    pub memory_progress_step: Option<i64>,
    #[serde(default = "default_speaker_selection_method")]
    pub speaker_selection_method: String,
    #[serde(default = "default_memory_type")]
    pub memory_type: String,
    #[serde(default = "default_group_config_overrides_json")]
    pub config_overrides: String,
    #[serde(default = "default_character_model_overrides_json")]
    pub character_model_overrides: String,
    #[serde(default)]
    pub group_chat_prompt_template_id: Option<String>,
    #[serde(default)]
    pub group_chat_roleplay_prompt_template_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupParticipation {
    pub id: String,
    pub session_id: String,
    pub character_id: String,
    pub speak_count: i64,
    pub last_spoke_turn: Option<i64>,
    pub last_spoke_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub speaker_character_id: Option<String>,
    pub turn_number: i64,
    pub created_at: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    #[serde(default)]
    pub first_token_ms: Option<i64>,
    #[serde(default)]
    pub tokens_per_second: Option<f64>,
    #[serde(default)]
    pub mtp_stats: Option<String>,
    pub selected_variant_id: Option<String>,
    pub is_pinned: i64,
    pub attachments: String,
    #[serde(default)]
    pub used_lorebook_entries: String,
    #[serde(default)]
    pub memory_refs: String,
    pub reasoning: Option<String>,
    pub selection_reasoning: Option<String>,
    pub model_id: Option<String>,
    #[serde(default)]
    pub gemini_content: Option<String>,
    #[serde(default)]
    pub usage_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupMessageVariant {
    pub id: String,
    pub message_id: String,
    pub content: String,
    pub speaker_character_id: Option<String>,
    pub created_at: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    #[serde(default)]
    pub first_token_ms: Option<i64>,
    #[serde(default)]
    pub tokens_per_second: Option<f64>,
    #[serde(default)]
    pub mtp_stats: Option<String>,
    pub reasoning: Option<String>,
    pub selection_reasoning: Option<String>,
    pub model_id: Option<String>,
    #[serde(default = "default_json_array")]
    pub attachments: String,
    #[serde(default)]
    pub gemini_content: Option<String>,
    #[serde(default)]
    pub usage_json: Option<String>,
}

fn default_json_array() -> String {
    "[]".to_string()
}
