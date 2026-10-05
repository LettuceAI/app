//! Local Kokoro model assets and text preparation.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroVariantRequest { pub variant: String, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroInventoryRequest { pub variant: String, pub selected_voice_id: Option<String>, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroArtifactView { #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))] pub byte_size: u64, pub blake3: String, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroInstalledVoiceView { pub id: String, pub artifact: KokoroArtifactView, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroInventory { pub variant: String, pub variant_allowed_on_platform: bool, pub model: Option<KokoroArtifactView>, pub config: Option<KokoroArtifactView>, pub tokenizer: Option<KokoroArtifactView>, pub tokenizer_config: Option<KokoroArtifactView>, pub installed_voices: Vec<KokoroInstalledVoiceView>, pub selected_voice_installed: Option<bool>, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroAvailableVoiceView { pub id: String, pub installed: bool, pub source_revision: String, #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))] pub byte_size: u64, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroVoicesInstallRequest { pub voice_ids: Vec<String>, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroVoiceRequest { pub voice_id: String, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroVoiceBlendInput { pub voice_id: String, pub weight: f32, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroBlendRequest { pub voices: Vec<KokoroVoiceBlendInput>, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroBlendView { pub voices: Vec<KokoroVoiceBlendInput>, pub style_rows: u32, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroVariantView { pub id: String, pub label: String, pub filename: String, pub size_mb: f32, pub mobile_supported: bool, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroPhonemizeRequest { pub variant: String, pub voice_id: String, pub text: String, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroTokenizePreviewRequest { pub variant: String, pub voice_blend: Vec<KokoroVoiceBlendInput>, pub text: String, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroPhonemizationView { pub normalized_text: String, pub effective_text: String, pub language: String, pub used_lexicon_entries: Vec<String>, pub segments: Vec<KokoroPhonemizationSegmentView>, pub token_ids: Vec<i32>, pub primary_voice_id: String, pub voice_blend: Vec<KokoroVoiceBlendInput>, pub lexicon_path: String, pub lexicon_entry_count: u32, pub token_count: u32, pub chunk_lengths: Vec<u32>, pub warnings: Vec<String>, }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct KokoroPhonemizationSegmentView { pub kind: String, pub source_text: String, pub ipa: String, pub token_ids: Vec<i32>, }
