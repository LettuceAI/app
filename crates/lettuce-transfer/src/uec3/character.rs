use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{ChatAppearance, Design, Hint, ImageRecommendation, LorebookBinding, Starter};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub name: String,
    pub nickname: Option<String>,
    pub description: Option<String>,
    pub definition: Option<String>,
    pub scenario: Option<String>,
    pub creator: Option<String>,
    pub creator_notes: Option<String>,
    pub creator_notes_localized: BTreeMap<String, String>,
    pub sources: Vec<String>,
    pub tags: Vec<String>,
    pub design: Design,
    pub media: CharacterMedia,
    pub appearance: Appearance,
    pub scenes: Vec<Scene>,
    pub default_scene: Option<String>,
    pub starters: Vec<Starter>,
    pub default_starter: Option<String>,
    pub behavior: CharacterBehavior,
    pub companion: Option<super::Companion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMedia {
    pub avatar: Option<String>,
    pub banner: Option<String>,
    pub background: Option<String>,
    pub avatar_crop: Option<super::Crop>,
    pub banner_crop: Option<super::Crop>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub format_version: u32,
    pub card_style: lettuce_characters::CardStyle,
    pub disable_gradient: bool,
    pub gradient_source: lettuce_characters::GradientSource,
    pub custom_gradient: CustomGradient,
    pub text_colors: TextColors,
    pub chat: ChatAppearance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_overrides: Option<std::collections::BTreeSet<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomGradient {
    pub enabled: bool,
    pub colors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextColors {
    pub primary: Option<String>,
    pub secondary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScenePart {
    Text { text: String },
    Image { image: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub key: String,
    pub format_version: u32,
    pub parts: Vec<ScenePart>,
    pub direction: Option<String>,
    pub background: Option<String>,
    pub variants: Vec<SceneVariant>,
    pub selected_variant: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneVariant {
    pub key: String,
    pub format_version: u32,
    pub parts: Vec<ScenePart>,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterBehavior {
    pub interaction_mode: lettuce_characters::InteractionMode,
    pub memory: lettuce_characters::MemoryPolicy,
    pub prompts: CharacterPrompts,
    pub model: Option<Hint>,
    pub voice: Voice,
    pub lorebooks: Vec<LorebookBinding>,
    pub image: Option<ImageRecommendation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterPrompts {
    pub direct: Option<String>,
    pub group_conversation: Option<String>,
    pub group_roleplay: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Voice {
    pub provider: Option<VoicePreference>,
    pub autoplay: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VoicePreference {
    Provider(ProviderVoice),
    Profile(VoiceProfile),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderVoice {
    pub kind: String,
    pub account: ProviderAccountHint,
    pub voice_id: String,
    pub model_id: Option<String>,
    pub voice_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProfile {
    pub voice_profile: Hint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderAccountHint {
    pub provider: String,
    pub label: String,
}
