use std::collections::BTreeSet;

use lettuce_types::{AssetId, CharacterId};
use serde::{Deserialize, Serialize};

use crate::ValidationError;
use crate::constants::{
    MAX_COLLECTION_ITEMS, validate_collection, validate_color, validate_finite, validate_non_blank,
    validate_optional_color, validate_text,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

impl Crop {
    pub fn new(x: f32, y: f32, scale: f32) -> Result<Self, ValidationError> {
        let crop = Self { x, y, scale };
        crop.validate()?;
        Ok(crop)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_finite("crop.x", self.x)?;
        validate_finite("crop.y", self.y)?;
        validate_finite("crop.scale", self.scale)?;
        if self.scale <= 0.0 {
            return Err(ValidationError::InvalidValue {
                field: "crop.scale",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardStyle {
    Circle,
    Banner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientSource {
    Base,
    Round,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ChatWidgetSlots {
    pub left: Vec<WidgetNode>,
    pub right: Vec<WidgetNode>,
}

impl ChatWidgetSlots {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_collection("chat_widget_slots.left", &self.left, MAX_COLLECTION_ITEMS)?;
        validate_collection("chat_widget_slots.right", &self.right, MAX_COLLECTION_ITEMS)?;
        for node in self.left.iter().chain(self.right.iter()) {
            node.validate()?;
        }
        Ok(())
    }

    /// Returns logical asset references in stable ID order. Unresolved import
    /// tokens are intentionally excluded because they are not asset IDs.
    #[must_use]
    pub fn referenced_asset_ids(&self) -> BTreeSet<AssetId> {
        self.left
            .iter()
            .chain(self.right.iter())
            .flat_map(WidgetNode::referenced_asset_ids)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetDesign {
    Default,
    Minimal,
    Solid,
    Outline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoxVariant {
    Default,
    Subtle,
    Info,
    Warning,
    Success,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetImageShape {
    Auto,
    Square,
    Wide,
    Circle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WidgetImageSource {
    CharacterAvatar,
    PersonaAvatar,
    LogicalAsset { asset_id: AssetId },
    UnresolvedLegacy { token: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectorKind {
    Persona,
    Model,
    AuthorNote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonAction {
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
#[serde(deny_unknown_fields)]
pub struct WidgetStat {
    pub id: String,
    pub label: String,
    pub value: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetSnippet {
    pub id: String,
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WidgetNode {
    Divider {
        id: String,
        design: Option<WidgetDesign>,
        style: Option<DividerStyle>,
    },
    Box {
        id: String,
        design: Option<WidgetDesign>,
        variant: Option<BoxVariant>,
        title: Option<String>,
        description: Option<String>,
        children: Vec<Self>,
    },
    CharacterInfo {
        id: String,
        design: Option<WidgetDesign>,
        character_id: Option<CharacterId>,
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
        kind: SelectorKind,
        title: Option<String>,
        description: Option<String>,
    },
    Button {
        id: String,
        design: Option<WidgetDesign>,
        action: ButtonAction,
        title: Option<String>,
        description: Option<String>,
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
        limit: Option<u16>,
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
        hour_format: Option<HourFormat>,
        show_seconds: Option<bool>,
        show_date: Option<bool>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DividerStyle {
    Line,
    Space,
}

impl WidgetNode {
    /// Returns this node's logical asset references and those of descendants.
    #[must_use]
    pub fn referenced_asset_ids(&self) -> BTreeSet<AssetId> {
        let mut asset_ids = BTreeSet::new();
        if let Self::Image {
            source: WidgetImageSource::LogicalAsset { asset_id },
            ..
        } = self
        {
            asset_ids.insert(*asset_id);
        }
        if let Self::Box { children, .. } = self {
            for child in children {
                asset_ids.extend(child.referenced_asset_ids());
            }
        }
        asset_ids
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_at_depth(0)
    }

    fn validate_at_depth(&self, depth: u8) -> Result<(), ValidationError> {
        if depth > 16 {
            return Err(ValidationError::Invariant {
                field: "widget.depth",
            });
        }
        let id = match self {
            Self::Divider { id, .. }
            | Self::Box { id, .. }
            | Self::CharacterInfo { id, .. }
            | Self::PersonaInfo { id, .. }
            | Self::ScratchPad { id, .. }
            | Self::Image { id, .. }
            | Self::Selector { id, .. }
            | Self::Button { id, .. }
            | Self::StatTracker { id, .. }
            | Self::QuickSnippets { id, .. }
            | Self::Dice { id, .. }
            | Self::Memory { id, .. }
            | Self::CompanionState { id, .. }
            | Self::SessionInfo { id, .. }
            | Self::AuthorNote { id, .. }
            | Self::Time { id, .. } => id,
        };
        validate_non_blank("widget.id", id)?;
        match self {
            Self::Box {
                children,
                title,
                description,
                ..
            } => {
                validate_collection("widget.children", children, MAX_COLLECTION_ITEMS)?;
                validate_optional_text(title.as_ref())?;
                validate_optional_text(description.as_ref())?;
                for child in children {
                    child.validate_at_depth(depth + 1)?;
                }
            }
            Self::ScratchPad {
                title,
                description,
                content,
                ..
            } => {
                validate_optional_text(title.as_ref())?;
                validate_optional_text(description.as_ref())?;
                validate_optional_text(content.as_ref())?;
            }
            Self::Image {
                title,
                description,
                source,
                ..
            } => {
                validate_optional_text(title.as_ref())?;
                validate_optional_text(description.as_ref())?;
                if let WidgetImageSource::UnresolvedLegacy { token } = source {
                    validate_non_blank("widget.image.unresolved_token", token)?;
                }
            }
            Self::Selector {
                title, description, ..
            }
            | Self::Button {
                title, description, ..
            }
            | Self::AuthorNote {
                title, description, ..
            }
            | Self::Dice {
                title, description, ..
            } => {
                validate_optional_text(title.as_ref())?;
                validate_optional_text(description.as_ref())?;
            }
            Self::StatTracker {
                title,
                description,
                stats,
                ..
            } => {
                validate_optional_text(title.as_ref())?;
                validate_optional_text(description.as_ref())?;
                validate_collection("widget.stats", stats, MAX_COLLECTION_ITEMS)?;
                for stat in stats {
                    validate_non_blank("widget.stat.id", &stat.id)?;
                    validate_non_blank("widget.stat.label", &stat.label)?;
                    if !stat.value.is_finite() {
                        return Err(ValidationError::NonFinite {
                            field: "widget.stat.value",
                        });
                    }
                    if let (Some(min), Some(max)) = (stat.min, stat.max) {
                        if !min.is_finite() || !max.is_finite() {
                            return Err(ValidationError::NonFinite {
                                field: "widget.stat.range",
                            });
                        }
                        if min > max {
                            return Err(ValidationError::InvalidValue {
                                field: "widget.stat.range",
                            });
                        }
                    }
                }
            }
            Self::QuickSnippets {
                title,
                description,
                snippets,
                ..
            } => {
                validate_optional_text(title.as_ref())?;
                validate_optional_text(description.as_ref())?;
                validate_collection("widget.snippets", snippets, MAX_COLLECTION_ITEMS)?;
                for snippet in snippets {
                    validate_non_blank("widget.snippet.id", &snippet.id)?;
                    validate_non_blank("widget.snippet.label", &snippet.label)?;
                    validate_text("widget.snippet.text", &snippet.text)?;
                }
            }
            Self::Memory { limit, .. } => {
                if limit.is_some_and(|value| !(1..=100).contains(&value)) {
                    return Err(ValidationError::InvalidValue {
                        field: "widget.memory.limit",
                    });
                }
            }
            Self::Time { .. }
            | Self::Divider { .. }
            | Self::CharacterInfo { .. }
            | Self::PersonaInfo { .. }
            | Self::CompanionState { .. }
            | Self::SessionInfo { .. } => {}
        }
        Ok(())
    }
}

fn validate_optional_text(value: Option<&String>) -> Result<(), ValidationError> {
    if let Some(value) = value {
        validate_text("widget.text", value)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontSize {
    Small,
    Medium,
    Large,
    Xlarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineSpacing {
    Tight,
    Normal,
    Relaxed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BubbleStyle {
    Bordered,
    Filled,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BubbleRadius {
    Sharp,
    Rounded,
    Pill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BubbleMaxWidth {
    Compact,
    Normal,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BubblePadding {
    Compact,
    Normal,
    Spacious,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampFormat {
    Relative,
    Time,
    Datetime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageHeaderPlacement {
    Inside,
    Above,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageInfoPlacement {
    BelowHeader,
    BelowHeaderOutside,
    InsideBubble,
    BelowBubble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageInfoSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageGap {
    Tight,
    Normal,
    Relaxed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarShape {
    Circle,
    Rounded,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatColumnWidth {
    Narrow,
    Normal,
    Wide,
    Xl,
    Full,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantBarAvatarShape {
    Round,
    Boxed,
    RoundedBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantBarBackground {
    Solid,
    Fading,
    Transparent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantBarHintPosition {
    Top,
    Bottom,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetCenterMode {
    Both,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserBubbleColor {
    Accent,
    Info,
    Secondary,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantBubbleColor {
    Neutral,
    Accent,
    Info,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BubbleBlur {
    None,
    Light,
    Medium,
    Heavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextMode {
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HourFormat {
    H12,
    H24,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatAppearanceV1 {
    pub format_version: u32,
    pub font_size: FontSize,
    pub line_spacing: LineSpacing,
    pub bubble_style: BubbleStyle,
    pub bubble_radius: BubbleRadius,
    pub bubble_max_width: BubbleMaxWidth,
    pub bubble_padding: BubblePadding,
    pub show_message_author: bool,
    pub show_message_timestamp: bool,
    pub timestamp_format: TimestampFormat,
    pub message_header_placement: MessageHeaderPlacement,
    pub show_message_model: bool,
    pub show_message_input_tokens: bool,
    pub show_message_output_tokens: bool,
    pub show_message_total_tokens: bool,
    pub show_message_ttft: bool,
    pub show_message_tokens_per_second: bool,
    pub show_message_mtp: bool,
    pub message_info_placement: MessageInfoPlacement,
    pub message_info_size: MessageInfoSize,
    pub message_gap: MessageGap,
    pub avatar_shape: AvatarShape,
    pub avatar_size: AvatarSize,
    pub chat_column_width: ChatColumnWidth,
    pub chat_column_width_px: Option<u16>,
    pub chat_column_align: Alignment,
    pub chat_header_moves: bool,
    pub chat_footer_moves: bool,
    pub participants_bar_enabled: bool,
    pub participants_bar_avatar_size: AvatarSize,
    pub participants_bar_avatar_shape: ParticipantBarAvatarShape,
    pub participants_bar_background: ParticipantBarBackground,
    pub participants_bar_gap: MessageGap,
    pub participants_bar_align: Alignment,
    pub participants_bar_hint_position: ParticipantBarHintPosition,
    pub chat_widget_area_enabled: bool,
    pub chat_widget_center_mode: WidgetCenterMode,
    pub chat_widget_slots: ChatWidgetSlots,
    pub user_bubble_color: UserBubbleColor,
    pub assistant_bubble_color: AssistantBubbleColor,
    pub user_bubble_color_hex: Option<String>,
    pub assistant_bubble_color_hex: Option<String>,
    pub footer_input_color_hex: Option<String>,
    pub message_text_color_hex: Option<String>,
    pub plain_text_color_hex: Option<String>,
    pub italic_text_color_hex: Option<String>,
    pub quoted_text_color_hex: Option<String>,
    pub inline_code_text_color_hex: Option<String>,
    pub transparent_header: bool,
    pub background_dim: f32,
    pub background_blur: f32,
    pub bubble_blur: BubbleBlur,
    pub bubble_opacity: f32,
    pub text_mode: TextMode,
}

impl Default for ChatAppearanceV1 {
    fn default() -> Self {
        Self {
            format_version: 1,
            font_size: FontSize::Medium,
            line_spacing: LineSpacing::Relaxed,
            bubble_style: BubbleStyle::Bordered,
            bubble_radius: BubbleRadius::Rounded,
            bubble_max_width: BubbleMaxWidth::Normal,
            bubble_padding: BubblePadding::Normal,
            show_message_author: false,
            show_message_timestamp: false,
            timestamp_format: TimestampFormat::Relative,
            message_header_placement: MessageHeaderPlacement::Inside,
            show_message_model: false,
            show_message_input_tokens: false,
            show_message_output_tokens: false,
            show_message_total_tokens: false,
            show_message_ttft: false,
            show_message_tokens_per_second: false,
            show_message_mtp: false,
            message_info_placement: MessageInfoPlacement::BelowBubble,
            message_info_size: MessageInfoSize::Small,
            message_gap: MessageGap::Relaxed,
            avatar_shape: AvatarShape::Circle,
            avatar_size: AvatarSize::Medium,
            chat_column_width: ChatColumnWidth::Full,
            chat_column_width_px: None,
            chat_column_align: Alignment::Center,
            chat_header_moves: false,
            chat_footer_moves: false,
            participants_bar_enabled: true,
            participants_bar_avatar_size: AvatarSize::Medium,
            participants_bar_avatar_shape: ParticipantBarAvatarShape::Round,
            participants_bar_background: ParticipantBarBackground::Fading,
            participants_bar_gap: MessageGap::Normal,
            participants_bar_align: Alignment::Left,
            participants_bar_hint_position: ParticipantBarHintPosition::Bottom,
            chat_widget_area_enabled: false,
            chat_widget_center_mode: WidgetCenterMode::Both,
            chat_widget_slots: ChatWidgetSlots::default(),
            user_bubble_color: UserBubbleColor::Accent,
            assistant_bubble_color: AssistantBubbleColor::Neutral,
            user_bubble_color_hex: None,
            assistant_bubble_color_hex: None,
            footer_input_color_hex: None,
            message_text_color_hex: None,
            plain_text_color_hex: None,
            italic_text_color_hex: None,
            quoted_text_color_hex: None,
            inline_code_text_color_hex: None,
            transparent_header: false,
            background_dim: 0.0,
            background_blur: 0.0,
            bubble_blur: BubbleBlur::None,
            bubble_opacity: 35.0,
            text_mode: TextMode::Auto,
        }
    }
}

impl ChatAppearanceV1 {
    /// Returns nested chat-widget logical asset references in stable ID order.
    #[must_use]
    pub fn referenced_asset_ids(&self) -> BTreeSet<AssetId> {
        self.chat_widget_slots.referenced_asset_ids()
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.format_version != 1 {
            return Err(ValidationError::UnsupportedVersion {
                field: "chat_appearance",
                version: self.format_version,
            });
        }
        if self.chat_column_width == ChatColumnWidth::Custom
            && !self
                .chat_column_width_px
                .is_some_and(|value| (400..=2400).contains(&value))
        {
            return Err(ValidationError::InvalidValue {
                field: "chat_column_width_px",
            });
        }
        for (field, value) in [
            ("background_dim", self.background_dim),
            ("background_blur", self.background_blur),
            ("bubble_opacity", self.bubble_opacity),
        ] {
            validate_finite(field, value)?;
        }
        if !(0.0..=80.0).contains(&self.background_dim)
            || !(0.0..=20.0).contains(&self.background_blur)
            || !(20.0..=100.0).contains(&self.bubble_opacity)
        {
            return Err(ValidationError::InvalidValue {
                field: "chat_appearance.range",
            });
        }
        for (field, value) in [
            ("user_bubble_color_hex", &self.user_bubble_color_hex),
            (
                "assistant_bubble_color_hex",
                &self.assistant_bubble_color_hex,
            ),
            ("footer_input_color_hex", &self.footer_input_color_hex),
            ("message_text_color_hex", &self.message_text_color_hex),
            ("plain_text_color_hex", &self.plain_text_color_hex),
            ("italic_text_color_hex", &self.italic_text_color_hex),
            ("quoted_text_color_hex", &self.quoted_text_color_hex),
            (
                "inline_code_text_color_hex",
                &self.inline_code_text_color_hex,
            ),
        ] {
            validate_optional_color(field, value.as_ref())?;
        }
        self.chat_widget_slots.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterPresentationV1 {
    pub format_version: u32,
    pub card_style: CardStyle,
    pub avatar_crop: Option<Crop>,
    pub banner_crop: Option<Crop>,
    pub disable_gradient: bool,
    pub gradient_source: GradientSource,
    pub custom_gradient_enabled: bool,
    pub custom_gradient_colors: Vec<String>,
    pub primary_text_color: Option<String>,
    pub secondary_text_color: Option<String>,
    pub chat_appearance: ChatAppearanceV1,
}

impl Default for CharacterPresentationV1 {
    fn default() -> Self {
        Self {
            format_version: 1,
            card_style: CardStyle::Circle,
            avatar_crop: None,
            banner_crop: None,
            disable_gradient: false,
            gradient_source: GradientSource::Base,
            custom_gradient_enabled: false,
            custom_gradient_colors: Vec::new(),
            primary_text_color: None,
            secondary_text_color: None,
            chat_appearance: ChatAppearanceV1::default(),
        }
    }
}

impl CharacterPresentationV1 {
    /// Returns logical presentation asset references, excluding unresolved
    /// legacy widget tokens.
    #[must_use]
    pub fn referenced_asset_ids(&self) -> BTreeSet<AssetId> {
        self.chat_appearance.referenced_asset_ids()
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.format_version != 1 {
            return Err(ValidationError::UnsupportedVersion {
                field: "presentation",
                version: self.format_version,
            });
        }
        if let Some(crop) = self.avatar_crop {
            crop.validate()?;
        }
        if let Some(crop) = self.banner_crop {
            crop.validate()?;
        }
        validate_collection("custom_gradient_colors", &self.custom_gradient_colors, 256)?;
        for color in &self.custom_gradient_colors {
            validate_color("custom_gradient_color", color)?;
        }
        validate_optional_color("primary_text_color", self.primary_text_color.as_ref())?;
        validate_optional_color("secondary_text_color", self.secondary_text_color.as_ref())?;
        self.chat_appearance.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CharacterPresentationV1, ChatAppearanceV1, Crop, WidgetImageShape, WidgetImageSource,
        WidgetNode,
    };
    use lettuce_types::AssetId;

    #[test]
    fn crop_rejects_non_positive_or_non_finite_scale() {
        assert!(Crop::new(0.0, 0.0, 0.0).is_err());
        assert!(Crop::new(0.0, 0.0, f32::NAN).is_err());
    }

    #[test]
    fn defaults_validate_and_round_trip_with_closed_json() {
        let presentation = CharacterPresentationV1::default();
        presentation
            .validate()
            .expect("default presentation should validate");
        let json = serde_json::to_string(&presentation).expect("presentation should serialize");
        let decoded: CharacterPresentationV1 =
            serde_json::from_str(&json).expect("presentation should decode");
        assert_eq!(decoded, presentation);
        assert!(
            serde_json::from_str::<ChatAppearanceV1>(r#"{"format_version":1,"unexpected":true}"#)
                .is_err()
        );
    }

    #[test]
    fn recursive_widgets_are_checked() {
        let node = WidgetNode::Box {
            id: "root".into(),
            design: None,
            variant: None,
            title: None,
            description: None,
            children: Vec::new(),
        };
        assert!(node.validate().is_ok());
    }

    #[test]
    fn referenced_asset_ids_are_recursive_unique_and_exclude_legacy_tokens() {
        let first = AssetId::new();
        let second = AssetId::new();
        let image = |id: &str, source| WidgetNode::Image {
            id: id.into(),
            design: None,
            title: None,
            description: None,
            source,
            shape: Some(WidgetImageShape::Square),
        };
        let nested = WidgetNode::Box {
            id: "box".into(),
            design: None,
            variant: None,
            title: None,
            description: None,
            children: vec![
                image("first", WidgetImageSource::LogicalAsset { asset_id: first }),
                WidgetNode::Box {
                    id: "inner".into(),
                    design: None,
                    variant: None,
                    title: None,
                    description: None,
                    children: vec![
                        image(
                            "duplicate",
                            WidgetImageSource::LogicalAsset { asset_id: first },
                        ),
                        image(
                            "second",
                            WidgetImageSource::LogicalAsset { asset_id: second },
                        ),
                        image(
                            "legacy",
                            WidgetImageSource::UnresolvedLegacy {
                                token: "old-avatar".into(),
                            },
                        ),
                    ],
                },
            ],
        };
        let mut appearance = ChatAppearanceV1::default();
        appearance.chat_widget_slots.left = vec![nested];
        appearance.chat_widget_slots.right = vec![image(
            "right-duplicate",
            WidgetImageSource::LogicalAsset { asset_id: second },
        )];
        let ids = CharacterPresentationV1 {
            chat_appearance: appearance,
            ..CharacterPresentationV1::default()
        }
        .referenced_asset_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&first));
        assert!(ids.contains(&second));
    }
}
