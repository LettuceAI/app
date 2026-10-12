use lettuce_characters::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatWidgetSlots {
    pub left: Vec<WidgetNode>,
    pub right: Vec<WidgetNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetStat {
    pub id: String,
    pub label: String,
    pub value: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetSnippet {
    pub id: String,
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum WidgetImageSource {
    CharacterAvatar,
    PersonaAvatar,
    LogicalAsset { asset_id: String },
    UnresolvedLegacy { token: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAppearance {
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

impl ChatAppearance {
    pub fn from_native(
        value: &lettuce_characters::ChatAppearanceV1,
        slots: ChatWidgetSlots,
    ) -> Self {
        Self {
            format_version: value.format_version,
            font_size: value.font_size,
            line_spacing: value.line_spacing,
            bubble_style: value.bubble_style,
            bubble_radius: value.bubble_radius,
            bubble_max_width: value.bubble_max_width,
            bubble_padding: value.bubble_padding,
            show_message_author: value.show_message_author,
            show_message_timestamp: value.show_message_timestamp,
            timestamp_format: value.timestamp_format,
            message_header_placement: value.message_header_placement,
            show_message_model: value.show_message_model,
            show_message_input_tokens: value.show_message_input_tokens,
            show_message_output_tokens: value.show_message_output_tokens,
            show_message_total_tokens: value.show_message_total_tokens,
            show_message_ttft: value.show_message_ttft,
            show_message_tokens_per_second: value.show_message_tokens_per_second,
            show_message_mtp: value.show_message_mtp,
            message_info_placement: value.message_info_placement,
            message_info_size: value.message_info_size,
            message_gap: value.message_gap,
            avatar_shape: value.avatar_shape,
            avatar_size: value.avatar_size,
            chat_column_width: value.chat_column_width,
            chat_column_width_px: value.chat_column_width_px,
            chat_column_align: value.chat_column_align,
            chat_header_moves: value.chat_header_moves,
            chat_footer_moves: value.chat_footer_moves,
            participants_bar_enabled: value.participants_bar_enabled,
            participants_bar_avatar_size: value.participants_bar_avatar_size,
            participants_bar_avatar_shape: value.participants_bar_avatar_shape,
            participants_bar_background: value.participants_bar_background,
            participants_bar_gap: value.participants_bar_gap,
            participants_bar_align: value.participants_bar_align,
            participants_bar_hint_position: value.participants_bar_hint_position,
            chat_widget_area_enabled: value.chat_widget_area_enabled,
            chat_widget_center_mode: value.chat_widget_center_mode,
            chat_widget_slots: slots,
            user_bubble_color: value.user_bubble_color,
            assistant_bubble_color: value.assistant_bubble_color,
            user_bubble_color_hex: value.user_bubble_color_hex.clone(),
            assistant_bubble_color_hex: value.assistant_bubble_color_hex.clone(),
            footer_input_color_hex: value.footer_input_color_hex.clone(),
            message_text_color_hex: value.message_text_color_hex.clone(),
            plain_text_color_hex: value.plain_text_color_hex.clone(),
            italic_text_color_hex: value.italic_text_color_hex.clone(),
            quoted_text_color_hex: value.quoted_text_color_hex.clone(),
            inline_code_text_color_hex: value.inline_code_text_color_hex.clone(),
            transparent_header: value.transparent_header,
            background_dim: value.background_dim,
            background_blur: value.background_blur,
            bubble_blur: value.bubble_blur,
            bubble_opacity: value.bubble_opacity,
            text_mode: value.text_mode,
        }
    }

    pub fn into_native(
        self,
        slots: lettuce_characters::ChatWidgetSlots,
    ) -> lettuce_characters::ChatAppearanceV1 {
        lettuce_characters::ChatAppearanceV1 {
            format_version: self.format_version,
            font_size: self.font_size,
            line_spacing: self.line_spacing,
            bubble_style: self.bubble_style,
            bubble_radius: self.bubble_radius,
            bubble_max_width: self.bubble_max_width,
            bubble_padding: self.bubble_padding,
            show_message_author: self.show_message_author,
            show_message_timestamp: self.show_message_timestamp,
            timestamp_format: self.timestamp_format,
            message_header_placement: self.message_header_placement,
            show_message_model: self.show_message_model,
            show_message_input_tokens: self.show_message_input_tokens,
            show_message_output_tokens: self.show_message_output_tokens,
            show_message_total_tokens: self.show_message_total_tokens,
            show_message_ttft: self.show_message_ttft,
            show_message_tokens_per_second: self.show_message_tokens_per_second,
            show_message_mtp: self.show_message_mtp,
            message_info_placement: self.message_info_placement,
            message_info_size: self.message_info_size,
            message_gap: self.message_gap,
            avatar_shape: self.avatar_shape,
            avatar_size: self.avatar_size,
            chat_column_width: self.chat_column_width,
            chat_column_width_px: self.chat_column_width_px,
            chat_column_align: self.chat_column_align,
            chat_header_moves: self.chat_header_moves,
            chat_footer_moves: self.chat_footer_moves,
            participants_bar_enabled: self.participants_bar_enabled,
            participants_bar_avatar_size: self.participants_bar_avatar_size,
            participants_bar_avatar_shape: self.participants_bar_avatar_shape,
            participants_bar_background: self.participants_bar_background,
            participants_bar_gap: self.participants_bar_gap,
            participants_bar_align: self.participants_bar_align,
            participants_bar_hint_position: self.participants_bar_hint_position,
            chat_widget_area_enabled: self.chat_widget_area_enabled,
            chat_widget_center_mode: self.chat_widget_center_mode,
            chat_widget_slots: slots,
            user_bubble_color: self.user_bubble_color,
            assistant_bubble_color: self.assistant_bubble_color,
            user_bubble_color_hex: self.user_bubble_color_hex,
            assistant_bubble_color_hex: self.assistant_bubble_color_hex,
            footer_input_color_hex: self.footer_input_color_hex,
            message_text_color_hex: self.message_text_color_hex,
            plain_text_color_hex: self.plain_text_color_hex,
            italic_text_color_hex: self.italic_text_color_hex,
            quoted_text_color_hex: self.quoted_text_color_hex,
            inline_code_text_color_hex: self.inline_code_text_color_hex,
            transparent_header: self.transparent_header,
            background_dim: self.background_dim,
            background_blur: self.background_blur,
            bubble_blur: self.bubble_blur,
            bubble_opacity: self.bubble_opacity,
            text_mode: self.text_mode,
        }
    }
}

impl WidgetNode {
    pub fn from_native(
        value: &lettuce_characters::WidgetNode,
        asset: &mut impl FnMut(lettuce_types::AssetId) -> Result<String, super::Error>,
        character: &mut impl FnMut(lettuce_types::CharacterId) -> Result<String, super::Error>,
    ) -> Result<Self, super::Error> {
        Ok(match value {
            lettuce_characters::WidgetNode::Divider { id, design, style } => Self::Divider {
                id: id.clone(),
                design: *design,
                style: *style,
            },
            lettuce_characters::WidgetNode::Box {
                id,
                design,
                variant,
                title,
                description,
                children,
            } => Self::Box {
                id: id.clone(),
                design: *design,
                variant: *variant,
                title: title.clone(),
                description: description.clone(),
                children: children
                    .iter()
                    .map(|child| Self::from_native(child, asset, character))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            lettuce_characters::WidgetNode::CharacterInfo {
                id,
                design,
                character_id,
            } => Self::CharacterInfo {
                id: id.clone(),
                design: *design,
                character_id: character_id.map(character).transpose()?,
            },
            lettuce_characters::WidgetNode::PersonaInfo { id, design } => Self::PersonaInfo {
                id: id.clone(),
                design: *design,
            },
            lettuce_characters::WidgetNode::ScratchPad {
                id,
                design,
                title,
                description,
                content,
            } => Self::ScratchPad {
                id: id.clone(),
                design: *design,
                title: title.clone(),
                description: description.clone(),
                content: content.clone(),
            },
            lettuce_characters::WidgetNode::Image {
                id,
                design,
                title,
                description,
                source,
                shape,
            } => Self::Image {
                id: id.clone(),
                design: *design,
                title: title.clone(),
                description: description.clone(),
                source: match source {
                    lettuce_characters::WidgetImageSource::CharacterAvatar => {
                        WidgetImageSource::CharacterAvatar
                    }
                    lettuce_characters::WidgetImageSource::PersonaAvatar => {
                        WidgetImageSource::PersonaAvatar
                    }
                    lettuce_characters::WidgetImageSource::LogicalAsset { asset_id } => {
                        WidgetImageSource::LogicalAsset {
                            asset_id: asset(*asset_id)?,
                        }
                    }
                    lettuce_characters::WidgetImageSource::UnresolvedLegacy { token } => {
                        WidgetImageSource::UnresolvedLegacy {
                            token: token.clone(),
                        }
                    }
                },
                shape: *shape,
            },
            lettuce_characters::WidgetNode::Selector {
                id,
                design,
                kind,
                title,
                description,
            } => Self::Selector {
                id: id.clone(),
                design: *design,
                kind: *kind,
                title: title.clone(),
                description: description.clone(),
            },
            lettuce_characters::WidgetNode::Button {
                id,
                design,
                action,
                title,
                description,
            } => Self::Button {
                id: id.clone(),
                design: *design,
                action: *action,
                title: title.clone(),
                description: description.clone(),
            },
            lettuce_characters::WidgetNode::StatTracker {
                id,
                design,
                title,
                description,
                stats,
            } => Self::StatTracker {
                id: id.clone(),
                design: *design,
                title: title.clone(),
                description: description.clone(),
                stats: stats
                    .iter()
                    .map(|item| WidgetStat {
                        id: item.id.clone(),
                        label: item.label.clone(),
                        value: item.value,
                        min: item.min,
                        max: item.max,
                    })
                    .collect(),
            },
            lettuce_characters::WidgetNode::QuickSnippets {
                id,
                design,
                title,
                description,
                snippets,
            } => Self::QuickSnippets {
                id: id.clone(),
                design: *design,
                title: title.clone(),
                description: description.clone(),
                snippets: snippets
                    .iter()
                    .map(|item| WidgetSnippet {
                        id: item.id.clone(),
                        label: item.label.clone(),
                        text: item.text.clone(),
                    })
                    .collect(),
            },
            lettuce_characters::WidgetNode::Dice {
                id,
                design,
                title,
                description,
                notation,
            } => Self::Dice {
                id: id.clone(),
                design: *design,
                title: title.clone(),
                description: description.clone(),
                notation: notation.clone(),
            },
            lettuce_characters::WidgetNode::Memory {
                id,
                design,
                title,
                limit,
            } => Self::Memory {
                id: id.clone(),
                design: *design,
                title: title.clone(),
                limit: *limit,
            },
            lettuce_characters::WidgetNode::CompanionState { id, design, title } => {
                Self::CompanionState {
                    id: id.clone(),
                    design: *design,
                    title: title.clone(),
                }
            }
            lettuce_characters::WidgetNode::SessionInfo { id, design, title } => {
                Self::SessionInfo {
                    id: id.clone(),
                    design: *design,
                    title: title.clone(),
                }
            }
            lettuce_characters::WidgetNode::AuthorNote {
                id,
                design,
                title,
                description,
            } => Self::AuthorNote {
                id: id.clone(),
                design: *design,
                title: title.clone(),
                description: description.clone(),
            },
            lettuce_characters::WidgetNode::Time {
                id,
                design,
                title,
                hour_format,
                show_seconds,
                show_date,
            } => Self::Time {
                id: id.clone(),
                design: *design,
                title: title.clone(),
                hour_format: *hour_format,
                show_seconds: *show_seconds,
                show_date: *show_date,
            },
        })
    }
    pub fn into_native(
        self,
        asset: &mut impl FnMut(&str) -> Result<lettuce_types::AssetId, super::Error>,
        character: &mut impl FnMut(&str) -> Result<Option<lettuce_types::CharacterId>, super::Error>,
    ) -> Result<lettuce_characters::WidgetNode, super::Error> {
        Ok(match self {
            Self::Divider { id, design, style } => {
                lettuce_characters::WidgetNode::Divider { id, design, style }
            }
            Self::Box {
                id,
                design,
                variant,
                title,
                description,
                children,
            } => lettuce_characters::WidgetNode::Box {
                id,
                design,
                variant,
                title,
                description,
                children: children
                    .into_iter()
                    .map(|child| child.into_native(asset, character))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            Self::CharacterInfo {
                id,
                design,
                character_id,
            } => lettuce_characters::WidgetNode::CharacterInfo {
                id,
                design,
                character_id: character_id
                    .as_deref()
                    .map(character)
                    .transpose()?
                    .flatten(),
            },
            Self::PersonaInfo { id, design } => {
                lettuce_characters::WidgetNode::PersonaInfo { id, design }
            }
            Self::ScratchPad {
                id,
                design,
                title,
                description,
                content,
            } => lettuce_characters::WidgetNode::ScratchPad {
                id,
                design,
                title,
                description,
                content,
            },
            Self::Image {
                id,
                design,
                title,
                description,
                source,
                shape,
            } => lettuce_characters::WidgetNode::Image {
                id,
                design,
                title,
                description,
                source: match source {
                    WidgetImageSource::CharacterAvatar => {
                        lettuce_characters::WidgetImageSource::CharacterAvatar
                    }
                    WidgetImageSource::PersonaAvatar => {
                        lettuce_characters::WidgetImageSource::PersonaAvatar
                    }
                    WidgetImageSource::LogicalAsset { asset_id } => {
                        lettuce_characters::WidgetImageSource::LogicalAsset {
                            asset_id: asset(&asset_id)?,
                        }
                    }
                    WidgetImageSource::UnresolvedLegacy { token } => {
                        lettuce_characters::WidgetImageSource::UnresolvedLegacy {
                            token: token.clone(),
                        }
                    }
                },
                shape,
            },
            Self::Selector {
                id,
                design,
                kind,
                title,
                description,
            } => lettuce_characters::WidgetNode::Selector {
                id,
                design,
                kind,
                title,
                description,
            },
            Self::Button {
                id,
                design,
                action,
                title,
                description,
            } => lettuce_characters::WidgetNode::Button {
                id,
                design,
                action,
                title,
                description,
            },
            Self::StatTracker {
                id,
                design,
                title,
                description,
                stats,
            } => lettuce_characters::WidgetNode::StatTracker {
                id,
                design,
                title,
                description,
                stats: stats
                    .iter()
                    .map(|item| lettuce_characters::WidgetStat {
                        id: item.id.clone(),
                        label: item.label.clone(),
                        value: item.value,
                        min: item.min,
                        max: item.max,
                    })
                    .collect(),
            },
            Self::QuickSnippets {
                id,
                design,
                title,
                description,
                snippets,
            } => lettuce_characters::WidgetNode::QuickSnippets {
                id,
                design,
                title,
                description,
                snippets: snippets
                    .iter()
                    .map(|item| lettuce_characters::WidgetSnippet {
                        id: item.id.clone(),
                        label: item.label.clone(),
                        text: item.text.clone(),
                    })
                    .collect(),
            },
            Self::Dice {
                id,
                design,
                title,
                description,
                notation,
            } => lettuce_characters::WidgetNode::Dice {
                id,
                design,
                title,
                description,
                notation,
            },
            Self::Memory {
                id,
                design,
                title,
                limit,
            } => lettuce_characters::WidgetNode::Memory {
                id,
                design,
                title,
                limit,
            },
            Self::CompanionState { id, design, title } => {
                lettuce_characters::WidgetNode::CompanionState { id, design, title }
            }
            Self::SessionInfo { id, design, title } => {
                lettuce_characters::WidgetNode::SessionInfo { id, design, title }
            }
            Self::AuthorNote {
                id,
                design,
                title,
                description,
            } => lettuce_characters::WidgetNode::AuthorNote {
                id,
                design,
                title,
                description,
            },
            Self::Time {
                id,
                design,
                title,
                hour_format,
                show_seconds,
                show_date,
            } => lettuce_characters::WidgetNode::Time {
                id,
                design,
                title,
                hour_format,
                show_seconds,
                show_date,
            },
        })
    }
}
