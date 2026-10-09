use crate::SettingsLlamaSamplerStage;
use serde::{Deserialize, Serialize};

fn optional_choice<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsUiCustomColors {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub surface: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub surface_el: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub fg: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub app_text: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub app_text_muted: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub app_text_subtle: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub accent: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub danger: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub warning: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub info: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub secondary: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub nav: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsUiCustomColorPreset {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiCustomColors, optional))]
    pub colors: Option<SettingsUiCustomColors>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = f64, optional))]
    pub settings_card_opacity: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = f64, optional))]
    pub created_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsUiAccessibilitySound {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub enabled: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = f64, optional))]
    pub volume: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiAccessibilitySettingsHapticIntensity {
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "heavy")]
    Heavy,
    #[serde(rename = "soft")]
    Soft,
    #[serde(rename = "rigid")]
    Rigid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsUiAccessibilitySettings {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiAccessibilitySound, optional))]
    pub send: Option<SettingsUiAccessibilitySound>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiAccessibilitySound, optional))]
    pub success: Option<SettingsUiAccessibilitySound>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiAccessibilitySound, optional))]
    pub failure: Option<SettingsUiAccessibilitySound>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub haptics: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiAccessibilitySettingsHapticIntensity, optional))]
    pub haptic_intensity: Option<SettingsUiAccessibilitySettingsHapticIntensity>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsFontSize {
    #[serde(rename = "small")]
    Small,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "large")]
    Large,
    #[serde(rename = "xlarge")]
    Xlarge,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsLineSpacing {
    #[serde(rename = "tight")]
    Tight,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "relaxed")]
    Relaxed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsBubbleStyle {
    #[serde(rename = "bordered")]
    Bordered,
    #[serde(rename = "filled")]
    Filled,
    #[serde(rename = "minimal")]
    Minimal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsBubbleRadius {
    #[serde(rename = "sharp")]
    Sharp,
    #[serde(rename = "rounded")]
    Rounded,
    #[serde(rename = "pill")]
    Pill,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsBubbleMaxWidth {
    #[serde(rename = "compact")]
    Compact,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "wide")]
    Wide,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsBubblePadding {
    #[serde(rename = "compact")]
    Compact,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "spacious")]
    Spacious,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsTimestampFormat {
    #[serde(rename = "relative")]
    Relative,
    #[serde(rename = "time")]
    Time,
    #[serde(rename = "datetime")]
    Datetime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsMessageHeaderPlacement {
    #[serde(rename = "inside")]
    Inside,
    #[serde(rename = "above")]
    Above,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsMessageInfoPlacement {
    #[serde(rename = "belowHeader")]
    BelowHeader,
    #[serde(rename = "belowHeaderOutside")]
    BelowHeaderOutside,
    #[serde(rename = "insideBubble")]
    InsideBubble,
    #[serde(rename = "belowBubble")]
    BelowBubble,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsMessageInfoSize {
    #[serde(rename = "small")]
    Small,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "large")]
    Large,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsMessageGap {
    #[serde(rename = "tight")]
    Tight,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "relaxed")]
    Relaxed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsAvatarShape {
    #[serde(rename = "circle")]
    Circle,
    #[serde(rename = "rounded")]
    Rounded,
    #[serde(rename = "hidden")]
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsAvatarSize {
    #[serde(rename = "small")]
    Small,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "large")]
    Large,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsChatColumnWidth {
    #[serde(rename = "narrow")]
    Narrow,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "wide")]
    Wide,
    #[serde(rename = "xl")]
    Xl,
    #[serde(rename = "full")]
    Full,
    #[serde(rename = "custom")]
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsChatColumnAlign {
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "center")]
    Center,
    #[serde(rename = "right")]
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsParticipantsBarAvatarSize {
    #[serde(rename = "small")]
    Small,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "large")]
    Large,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsParticipantsBarAvatarShape {
    #[serde(rename = "round")]
    Round,
    #[serde(rename = "boxed")]
    Boxed,
    #[serde(rename = "rounded_box")]
    RoundedBox,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsParticipantsBarBackground {
    #[serde(rename = "solid")]
    Solid,
    #[serde(rename = "fading")]
    Fading,
    #[serde(rename = "transparent")]
    Transparent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsParticipantsBarGap {
    #[serde(rename = "tight")]
    Tight,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "relaxed")]
    Relaxed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsParticipantsBarAlign {
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "center")]
    Center,
    #[serde(rename = "right")]
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsParticipantsBarHintPosition {
    #[serde(rename = "top")]
    Top,
    #[serde(rename = "bottom")]
    Bottom,
    #[serde(rename = "hidden")]
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsChatWidgetCenterMode {
    #[serde(rename = "both")]
    Both,
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "right")]
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsUiChatAppearanceSettingsChatWidgetSlots {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = Vec<SettingsWidgetNode>, optional))]
    pub left: Option<Vec<SettingsWidgetNode>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = Vec<SettingsWidgetNode>, optional))]
    pub right: Option<Vec<SettingsWidgetNode>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsUserBubbleColor {
    #[serde(rename = "accent")]
    Accent,
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "secondary")]
    Secondary,
    #[serde(rename = "warning")]
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsAssistantBubbleColor {
    #[serde(rename = "neutral")]
    Neutral,
    #[serde(rename = "accent")]
    Accent,
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "secondary")]
    Secondary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsBubbleBlur {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "heavy")]
    Heavy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatAppearanceSettingsTextMode {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsUiChatAppearanceSettings {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsFontSize, optional))]
    pub font_size: Option<SettingsUiChatAppearanceSettingsFontSize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsLineSpacing, optional))]
    pub line_spacing: Option<SettingsUiChatAppearanceSettingsLineSpacing>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsBubbleStyle, optional))]
    pub bubble_style: Option<SettingsUiChatAppearanceSettingsBubbleStyle>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsBubbleRadius, optional))]
    pub bubble_radius: Option<SettingsUiChatAppearanceSettingsBubbleRadius>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsBubbleMaxWidth, optional))]
    pub bubble_max_width: Option<SettingsUiChatAppearanceSettingsBubbleMaxWidth>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsBubblePadding, optional))]
    pub bubble_padding: Option<SettingsUiChatAppearanceSettingsBubblePadding>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub show_message_author: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub show_message_timestamp: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsTimestampFormat, optional))]
    pub timestamp_format: Option<SettingsUiChatAppearanceSettingsTimestampFormat>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsMessageHeaderPlacement, optional))]
    pub message_header_placement: Option<SettingsUiChatAppearanceSettingsMessageHeaderPlacement>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub show_message_model: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub show_message_input_tokens: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub show_message_output_tokens: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub show_message_total_tokens: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub show_message_ttft: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub show_message_tokens_per_second: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub show_message_mtp: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsMessageInfoPlacement, optional))]
    pub message_info_placement: Option<SettingsUiChatAppearanceSettingsMessageInfoPlacement>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsMessageInfoSize, optional))]
    pub message_info_size: Option<SettingsUiChatAppearanceSettingsMessageInfoSize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsMessageGap, optional))]
    pub message_gap: Option<SettingsUiChatAppearanceSettingsMessageGap>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsAvatarShape, optional))]
    pub avatar_shape: Option<SettingsUiChatAppearanceSettingsAvatarShape>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsAvatarSize, optional))]
    pub avatar_size: Option<SettingsUiChatAppearanceSettingsAvatarSize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsChatColumnWidth, optional))]
    pub chat_column_width: Option<SettingsUiChatAppearanceSettingsChatColumnWidth>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = f64, optional))]
    pub chat_column_width_px: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsChatColumnAlign, optional))]
    pub chat_column_align: Option<SettingsUiChatAppearanceSettingsChatColumnAlign>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub chat_header_moves: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub chat_footer_moves: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub participants_bar_enabled: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsParticipantsBarAvatarSize, optional))]
    pub participants_bar_avatar_size:
        Option<SettingsUiChatAppearanceSettingsParticipantsBarAvatarSize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsParticipantsBarAvatarShape, optional))]
    pub participants_bar_avatar_shape:
        Option<SettingsUiChatAppearanceSettingsParticipantsBarAvatarShape>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsParticipantsBarBackground, optional))]
    pub participants_bar_background:
        Option<SettingsUiChatAppearanceSettingsParticipantsBarBackground>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsParticipantsBarGap, optional))]
    pub participants_bar_gap: Option<SettingsUiChatAppearanceSettingsParticipantsBarGap>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsParticipantsBarAlign, optional))]
    pub participants_bar_align: Option<SettingsUiChatAppearanceSettingsParticipantsBarAlign>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsParticipantsBarHintPosition, optional))]
    pub participants_bar_hint_position:
        Option<SettingsUiChatAppearanceSettingsParticipantsBarHintPosition>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub chat_widget_area_enabled: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsChatWidgetCenterMode, optional))]
    pub chat_widget_center_mode: Option<SettingsUiChatAppearanceSettingsChatWidgetCenterMode>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsChatWidgetSlots, optional))]
    pub chat_widget_slots: Option<SettingsUiChatAppearanceSettingsChatWidgetSlots>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsUserBubbleColor, optional))]
    pub user_bubble_color: Option<SettingsUiChatAppearanceSettingsUserBubbleColor>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsAssistantBubbleColor, optional))]
    pub assistant_bubble_color: Option<SettingsUiChatAppearanceSettingsAssistantBubbleColor>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub user_bubble_color_hex: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub assistant_bubble_color_hex: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub footer_input_color_hex: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub message_text_color_hex: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub plain_text_color_hex: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub italic_text_color_hex: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub quoted_text_color_hex: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub inline_code_text_color_hex: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = bool, optional))]
    pub transparent_header: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = f64, optional))]
    pub background_dim: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = f64, optional))]
    pub background_blur: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsBubbleBlur, optional))]
    pub bubble_blur: Option<SettingsUiChatAppearanceSettingsBubbleBlur>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = f64, optional))]
    pub bubble_opacity: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = SettingsUiChatAppearanceSettingsTextMode, optional))]
    pub text_mode: Option<SettingsUiChatAppearanceSettingsTextMode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsUiLlamaSamplerPreset {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = String, optional))]
    pub name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_choice"
    )]
    #[cfg_attr(feature = "specta", specta(type = Vec<SettingsLlamaSamplerStage>, optional))]
    pub stages: Option<Vec<SettingsLlamaSamplerStage>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiTheme {
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiChatsViewMode {
    #[serde(rename = "hero")]
    Hero,
    #[serde(rename = "gallery")]
    Gallery,
    #[serde(rename = "list")]
    List,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiGroupChatsViewMode {
    #[serde(rename = "classic")]
    Classic,
    #[serde(rename = "detailed")]
    Detailed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiNavigationStyle {
    #[serde(rename = "bottom")]
    Bottom,
    #[serde(rename = "bottomLabels")]
    BottomLabels,
    #[serde(rename = "dock")]
    Dock,
    #[serde(rename = "sidebar")]
    Sidebar,
    #[serde(rename = "floatingSidebar")]
    FloatingSidebar,
    #[serde(rename = "header")]
    Header,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiNavigationSide {
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "right")]
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiHeaderStyle {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "attached")]
    Attached,
    #[serde(rename = "floating")]
    Floating,
    #[serde(rename = "inline")]
    Inline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiNavItemsItem {
    #[serde(rename = "chats")]
    Chats,
    #[serde(rename = "groups")]
    Groups,
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "discover")]
    Discover,
    #[serde(rename = "library")]
    Library,
    #[serde(rename = "search")]
    Search,
    #[serde(rename = "settings")]
    Settings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiNavAlign {
    #[serde(rename = "start")]
    Start,
    #[serde(rename = "center")]
    Center,
    #[serde(rename = "end")]
    End,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsUiNavEdge {
    #[serde(rename = "top")]
    Top,
    #[serde(rename = "bottom")]
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "key", deny_unknown_fields)]
pub enum UiPreferenceChange {
    #[serde(rename = "theme")]
    Theme { value: Option<SettingsUiTheme> },
    #[serde(rename = "settingsCardOpacity")]
    SettingsCardOpacity { value: Option<f64> },
    #[serde(rename = "customColors")]
    CustomColors {
        value: Option<SettingsUiCustomColors>,
    },
    #[serde(rename = "customColorPresets")]
    CustomColorPresets {
        value: Option<Vec<SettingsUiCustomColorPreset>>,
    },
    #[serde(rename = "chatsViewMode")]
    ChatsViewMode {
        value: Option<SettingsUiChatsViewMode>,
    },
    #[serde(rename = "groupChatsViewMode")]
    GroupChatsViewMode {
        value: Option<SettingsUiGroupChatsViewMode>,
    },
    #[serde(rename = "accessibility")]
    Accessibility {
        value: Option<SettingsUiAccessibilitySettings>,
    },
    #[serde(rename = "navigationStyle")]
    NavigationStyle {
        value: Option<SettingsUiNavigationStyle>,
    },
    #[serde(rename = "navigationSide")]
    NavigationSide {
        value: Option<SettingsUiNavigationSide>,
    },
    #[serde(rename = "headerStyle")]
    HeaderStyle {
        value: Option<SettingsUiHeaderStyle>,
    },
    #[serde(rename = "navItems")]
    NavItems {
        value: Option<Vec<SettingsUiNavItemsItem>>,
    },
    #[serde(rename = "navAlign")]
    NavAlign { value: Option<SettingsUiNavAlign> },
    #[serde(rename = "navEdge")]
    NavEdge { value: Option<SettingsUiNavEdge> },
    #[serde(rename = "chatAppearance")]
    ChatAppearance {
        value: Option<SettingsUiChatAppearanceSettings>,
    },
    #[serde(rename = "llamaSamplerPresets")]
    LlamaSamplerPresets {
        value: Option<Vec<SettingsUiLlamaSamplerPreset>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SettingsWidgetNode {
    Divider {
        id: String,
        design: Option<WidgetDesign>,
        style: Option<WidgetDividerStyle>,
    },
    Box {
        id: String,
        design: Option<WidgetDesign>,
        variant: Option<WidgetBoxVariant>,
        title: Option<String>,
        description: Option<String>,
        children: Vec<SettingsWidgetNode>,
    },
    CharacterInfo {
        id: String,
        design: Option<WidgetDesign>,
        #[serde(rename = "characterId")]
        character_id: Option<String>,
    },
    PersonaInfo {
        id: String,
        design: Option<WidgetDesign>,
    },
    ScratchPad {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
        description: Option<String>,
        content: Option<String>,
    },
    Image {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
        description: Option<String>,
        source: WidgetImageSource,
        shape: Option<WidgetImageShape>,
    },
    Selector {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
        description: Option<String>,
        kind: WidgetSelectorKind,
    },
    Button {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
        description: Option<String>,
        action: WidgetButtonAction,
    },
    StatTracker {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
        description: Option<String>,
        stats: Vec<WidgetStat>,
    },
    QuickSnippets {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
        description: Option<String>,
        snippets: Vec<WidgetSnippet>,
    },
    Dice {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
        description: Option<String>,
        notation: Option<String>,
    },
    Memory {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
        limit: Option<f64>,
    },
    CompanionState {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
    },
    SessionInfo {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
    },
    AuthorNote {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
        description: Option<String>,
    },
    Time {
        id: String,
        design: Option<WidgetDesign>,
        title: Option<String>,
        #[serde(rename = "hourFormat")]
        hour_format: Option<WidgetHourFormat>,
        #[serde(rename = "showSeconds")]
        show_seconds: Option<bool>,
        #[serde(rename = "showDate")]
        show_date: Option<bool>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WidgetImageSource {
    CharacterAvatar,
    PersonaAvatar,
    Library { path: String },
    Upload { path: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct WidgetStat {
    pub id: String,
    pub label: String,
    pub value: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct WidgetSnippet {
    pub id: String,
    pub label: String,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum WidgetDesign {
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "minimal")]
    Minimal,
    #[serde(rename = "solid")]
    Solid,
    #[serde(rename = "outline")]
    Outline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum WidgetDividerStyle {
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "space")]
    Space,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum WidgetBoxVariant {
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "subtle")]
    Subtle,
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "danger")]
    Danger,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum WidgetImageShape {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "square")]
    Square,
    #[serde(rename = "wide")]
    Wide,
    #[serde(rename = "circle")]
    Circle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum WidgetSelectorKind {
    #[serde(rename = "persona")]
    Persona,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "author_note")]
    AuthorNote,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum WidgetButtonAction {
    #[serde(rename = "regenerate")]
    Regenerate,
    #[serde(rename = "swap_places")]
    SwapPlaces,
    #[serde(rename = "new_session")]
    NewSession,
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "abort")]
    Abort,
    #[serde(rename = "view_history")]
    ViewHistory,
    #[serde(rename = "open_memories")]
    OpenMemories,
    #[serde(rename = "open_search")]
    OpenSearch,
    #[serde(rename = "toggle_voice_autoplay")]
    ToggleVoiceAutoplay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum WidgetHourFormat {
    #[serde(rename = "12h")]
    Twelve,
    #[serde(rename = "24h")]
    TwentyFour,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_optional_choices_reject_explicit_null() {
        assert!(
            serde_json::from_value::<SettingsUiCustomColors>(serde_json::json!({"accent":null}))
                .is_err()
        );
        assert!(serde_json::from_value::<SettingsUiCustomColors>(serde_json::json!({})).is_ok());
    }
}
