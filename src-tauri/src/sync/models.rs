use serde::{Deserialize, Serialize};

fn default_speaker_selection_method() -> String {
    "llm".to_string()
}

fn default_memory_type() -> String {
    "manual".to_string()
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
    pub is_default: i64, // Boolean as integer
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
pub struct AudioProvider {
    pub id: String,
    pub provider_type: String,
    pub label: String,
    pub api_key: Option<String>,
    pub project_id: Option<String>,
    pub location: Option<String>,
    pub base_url: Option<String>,
    pub request_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
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
    #[serde(default = "default_speaker_selection_method")]
    pub speaker_selection_method: String,
    #[serde(default = "default_memory_type")]
    pub memory_type: String,
}

// Layer 2: Lorebooks

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
    pub keywords: String, // JSON string
    pub case_sensitive: i64,
    pub content: String,
    pub priority: i32,
    pub display_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

// Layer 3: Characters

#[derive(Debug, Serialize, Deserialize)]
pub struct Character {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub avatar_crop_x: Option<f64>,
    pub avatar_crop_y: Option<f64>,
    pub avatar_crop_scale: Option<f64>,
    #[serde(default)]
    pub design_description: Option<String>,
    #[serde(default)]
    pub design_reference_image_ids: Option<String>,
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
    #[serde(default)]
    pub fallback_model_id: Option<String>,
    pub memory_type: String,
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
pub struct CharacterLorebookLink {
    pub character_id: String,
    pub lorebook_id: String,
    pub enabled: i64,
    pub display_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

// Layer 4: Sessions

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub character_id: String,
    pub title: String,
    #[serde(default)]
    pub background_image_path: Option<String>,
    pub system_prompt: Option<String>,
    pub selected_scene_id: Option<String>,
    #[serde(default)]
    pub prompt_template_id: Option<String>,
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
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
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

// Layer 5: Group Sessions

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
    pub selected_variant_id: Option<String>,
    pub is_pinned: i64,
    pub attachments: String,
    #[serde(default)]
    pub used_lorebook_entries: String,
    pub reasoning: Option<String>,
    pub selection_reasoning: Option<String>,
    pub model_id: Option<String>,
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
    pub reasoning: Option<String>,
    pub selection_reasoning: Option<String>,
    pub model_id: Option<String>,
}
