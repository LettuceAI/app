use std::collections::BTreeMap;

use crate::LibraryCrop;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryCardStyle {
    Circle,
    Banner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryGradientSource {
    Base,
    Round,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryChatWidgetSlots {
    pub left: Vec<LibraryWidgetNode>,
    pub right: Vec<LibraryWidgetNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryWidgetDesign {
    Default,
    Minimal,
    Solid,
    Outline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryBoxVariant {
    Default,
    Subtle,
    Info,
    Warning,
    Success,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryWidgetImageShape {
    Auto,
    Square,
    Wide,
    Circle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LibraryWidgetImageSource {
    CharacterAvatar,
    PersonaAvatar,
    LogicalAsset { asset_id: String },
    UnresolvedLegacy { token: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibrarySelectorKind {
    Persona,
    Model,
    AuthorNote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryButtonAction {
    Regenerate,
    SwapPlaces,
    NewSession,
    Continue,
    Abort,
    ViewHistory,
    OpenMemories,
    OpenSearch,
    ToggleVoiceAutoplay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryWidgetStat {
    pub id: String,
    pub label: String,
    pub value: f64,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub min: Option<f64>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryWidgetSnippet {
    pub id: String,
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LibraryWidgetNode {
    Divider {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryDividerStyle>))]
        style: Option<LibraryDividerStyle>,
    },
    Box {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryBoxVariant>))]
        variant: Option<LibraryBoxVariant>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        description: Option<String>,
        children: Vec<LibraryWidgetNode>,
    },
    CharacterInfo {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        character_id: Option<String>,
    },
    PersonaInfo {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
    },
    ScratchPad {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        description: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        content: Option<String>,
    },
    Image {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        description: Option<String>,
        source: LibraryWidgetImageSource,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetImageShape>))]
        shape: Option<LibraryWidgetImageShape>,
    },
    Selector {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        kind: LibrarySelectorKind,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        description: Option<String>,
    },
    Button {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        action: LibraryButtonAction,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        description: Option<String>,
    },
    StatTracker {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        description: Option<String>,
        stats: Vec<LibraryWidgetStat>,
    },
    QuickSnippets {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        description: Option<String>,
        snippets: Vec<LibraryWidgetSnippet>,
    },
    Dice {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        description: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        notation: Option<String>,
    },
    Memory {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<u16>))]
        limit: Option<u16>,
    },
    CompanionState {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
    },
    SessionInfo {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
    },
    AuthorNote {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        description: Option<String>,
    },
    Time {
        id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryWidgetDesign>))]
        design: Option<LibraryWidgetDesign>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        title: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<LibraryHourFormat>))]
        hour_format: Option<LibraryHourFormat>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<bool>))]
        show_seconds: Option<bool>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<bool>))]
        show_date: Option<bool>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryDividerStyle {
    Line,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryFontSize {
    Small,
    Medium,
    Large,
    Xlarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryLineSpacing {
    Tight,
    Normal,
    Relaxed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryBubbleStyle {
    Bordered,
    Filled,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryBubbleRadius {
    Sharp,
    Rounded,
    Pill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryBubbleMaxWidth {
    Compact,
    Normal,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryBubblePadding {
    Compact,
    Normal,
    Spacious,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryTimestampFormat {
    Relative,
    Time,
    Datetime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryMessageHeaderPlacement {
    Inside,
    Above,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryMessageInfoPlacement {
    BelowHeader,
    BelowHeaderOutside,
    InsideBubble,
    BelowBubble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryMessageInfoSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryMessageGap {
    Tight,
    Normal,
    Relaxed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryAvatarShape {
    Circle,
    Rounded,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryAvatarSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryChatColumnWidth {
    Narrow,
    Normal,
    Wide,
    Xl,
    Full,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryParticipantBarAvatarShape {
    Round,
    Boxed,
    RoundedBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryParticipantBarBackground {
    Solid,
    Fading,
    Transparent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryParticipantBarHintPosition {
    Top,
    Bottom,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryWidgetCenterMode {
    Both,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryUserBubbleColor {
    Accent,
    Info,
    Secondary,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryAssistantBubbleColor {
    Neutral,
    Accent,
    Info,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryBubbleBlur {
    None,
    Light,
    Medium,
    Heavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryTextMode {
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryHourFormat {
    H12,
    H24,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryChatAppearanceV1 {
    pub format_version: u32,
    pub font_size: LibraryFontSize,
    pub line_spacing: LibraryLineSpacing,
    pub bubble_style: LibraryBubbleStyle,
    pub bubble_radius: LibraryBubbleRadius,
    pub bubble_max_width: LibraryBubbleMaxWidth,
    pub bubble_padding: LibraryBubblePadding,
    pub show_message_author: bool,
    pub show_message_timestamp: bool,
    pub timestamp_format: LibraryTimestampFormat,
    pub message_header_placement: LibraryMessageHeaderPlacement,
    pub show_message_model: bool,
    pub show_message_input_tokens: bool,
    pub show_message_output_tokens: bool,
    pub show_message_total_tokens: bool,
    pub show_message_ttft: bool,
    pub show_message_tokens_per_second: bool,
    pub show_message_mtp: bool,
    pub message_info_placement: LibraryMessageInfoPlacement,
    pub message_info_size: LibraryMessageInfoSize,
    pub message_gap: LibraryMessageGap,
    pub avatar_shape: LibraryAvatarShape,
    pub avatar_size: LibraryAvatarSize,
    pub chat_column_width: LibraryChatColumnWidth,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<u16>))]
    pub chat_column_width_px: Option<u16>,
    pub chat_column_align: LibraryAlignment,
    pub chat_header_moves: bool,
    pub chat_footer_moves: bool,
    pub participants_bar_enabled: bool,
    pub participants_bar_avatar_size: LibraryAvatarSize,
    pub participants_bar_avatar_shape: LibraryParticipantBarAvatarShape,
    pub participants_bar_background: LibraryParticipantBarBackground,
    pub participants_bar_gap: LibraryMessageGap,
    pub participants_bar_align: LibraryAlignment,
    pub participants_bar_hint_position: LibraryParticipantBarHintPosition,
    pub chat_widget_area_enabled: bool,
    pub chat_widget_center_mode: LibraryWidgetCenterMode,
    pub chat_widget_slots: LibraryChatWidgetSlots,
    pub user_bubble_color: LibraryUserBubbleColor,
    pub assistant_bubble_color: LibraryAssistantBubbleColor,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub user_bubble_color_hex: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub assistant_bubble_color_hex: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub footer_input_color_hex: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub message_text_color_hex: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub plain_text_color_hex: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub italic_text_color_hex: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub quoted_text_color_hex: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub inline_code_text_color_hex: Option<String>,
    pub transparent_header: bool,
    pub background_dim: f32,
    pub background_blur: f32,
    pub bubble_blur: LibraryBubbleBlur,
    pub bubble_opacity: f32,
    pub text_mode: LibraryTextMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryCharacterPresentationV1 {
    pub format_version: u32,
    pub card_style: LibraryCardStyle,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<LibraryCrop>))]
    pub avatar_crop: Option<LibraryCrop>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<LibraryCrop>))]
    pub banner_crop: Option<LibraryCrop>,
    pub disable_gradient: bool,
    pub gradient_source: LibraryGradientSource,
    pub custom_gradient_enabled: bool,
    pub custom_gradient_colors: Vec<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub primary_text_color: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub secondary_text_color: Option<String>,
    pub chat_appearance: LibraryChatAppearanceV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryCharacterProfile {
    pub name: String,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub nickname: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub description: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub definition: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub design_description: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub scenario: Option<String>,
    pub rules: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryCharacterProvenance {
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub creator: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub creator_notes: Option<String>,
    pub localized_creator_notes: BTreeMap<String, String>,
    pub sources: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LibraryVoicePreference {
    VoiceProfile(String),
    Provider {
        provider_id: String,
        voice_id: String,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        model_id: Option<String>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[cfg_attr(feature = "specta", specta(type = Option<String>))]
        voice_name: Option<String>,
    },
    UnresolvedLegacy(LibraryLegacyVoiceLocatorV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryLegacyVoiceLocatorV1 {
    pub locator: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryCharacterDefaults {
    pub interaction_mode: LibraryInteractionMode,
    pub memory_policy: LibraryMemoryPolicy,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub model_profile_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub default_scene_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub default_starter_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub direct_prompt_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub group_conversation_prompt_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub group_roleplay_prompt_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<LibraryVoicePreference>))]
    pub voice: Option<LibraryVoicePreference>,
    pub voice_autoplay: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<crate::CompanionSoulConfigView>))]
    pub companion_soul: Option<crate::CompanionSoulConfigView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryCharacterMediaSlot {
    AvatarOriginal,
    Banner,
    Background,
    DesignReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryCharacterMediaLink {
    pub asset_id: String,
    pub slot: LibraryCharacterMediaSlot,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LibraryScenePart {
    Text { text: String },
    InlineAsset { link_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibrarySceneDocumentV1 {
    pub format_version: u32,
    pub parts: Vec<LibraryScenePart>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibrarySceneAssetSlot {
    Background,
    Inline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibrarySceneAssetLink {
    pub id: String,
    pub asset_id: String,
    pub slot: LibrarySceneAssetSlot,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibrarySceneDraft {
    pub id: String,
    pub ordinal: u32,
    pub content: LibrarySceneDocumentV1,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub direction: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub selected_variant_id: Option<String>,
    pub assets: Vec<LibrarySceneAssetLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibrarySceneVariantDraft {
    pub id: String,
    pub scene_id: String,
    pub ordinal: u32,
    pub content: LibrarySceneDocumentV1,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryStarterRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryStarterMessage {
    pub id: String,
    pub role: LibraryStarterRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryStarterDraft {
    pub id: String,
    pub name: String,
    pub ordinal: u32,
    pub messages: Vec<LibraryStarterMessage>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub scene_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub prompt_id: Option<String>,
    pub lorebooks: LibraryLorebookSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryInteractionMode {
    Roleplay,
    Companion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LibraryMemoryPolicy {
    Manual,
    Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LibraryLorebookSelection {
    Inherit,
    Explicit(Vec<String>),
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterDraft {
    pub profile: LibraryCharacterProfile,
    pub provenance: LibraryCharacterProvenance,
    pub defaults: LibraryCharacterDefaults,
    pub media: Vec<LibraryCharacterMediaLink>,
    pub presentation: LibraryCharacterPresentationV1,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<crate::LibraryImageRecommendation>))]
    pub image_recommendation: Option<crate::LibraryImageRecommendation>,
    pub scenes: Vec<LibrarySceneDraft>,
    pub variants: Vec<LibrarySceneVariantDraft>,
    pub starters: Vec<LibraryStarterDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterView {
    pub id: String,
    pub archived: bool,
    pub draft: CharacterDraft,
    pub assets: Vec<crate::AssetRef>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterGetRequest {
    pub character_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterSaveRequest {
    pub character_id: Option<String>,
    pub draft: CharacterDraft,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expected_revision: Option<u64>,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterLifecycleRequest {
    pub character_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}
