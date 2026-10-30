import { useTranslation } from "react-i18next";
import { Button, ListGroup, ListRow, LoadingState, useTheme } from "@lettuceai/crisp";
import type { GroupDraft_Serialize } from "@/api/generated/bindings";
import { usePreferences, type ChatLook } from "@/entities/preferences";
import { ChatPreview, ChatSettings } from "@/features/settings";
import { FailureState } from "@/shared/ui/FailureState";
import { appearanceFieldKeys, appearanceFields, changeGroupAppearance, groupAppearance } from "../data/appearance";

const labels = {
  font_size: "textSize", line_spacing: "lineSpacing", bubble_style: "style", bubble_radius: "corners", bubble_max_width: "width", bubble_padding: "padding",
  show_message_author: "showNames", show_message_timestamp: "showTime", timestamp_format: "timeFormat", message_header_placement: "placement",
  show_message_model: "details.model", show_message_input_tokens: "details.input", show_message_output_tokens: "details.output", show_message_total_tokens: "details.total", show_message_ttft: "details.ttft", show_message_tokens_per_second: "details.tps", show_message_mtp: "details.mtp", message_info_placement: "placement", message_info_size: "size",
  message_gap: "spaceBetweenMessages", avatar_shape: "avatars", avatar_size: "avatarSize", chat_column_width: "width", chat_column_width_px: "customWidth", chat_column_align: "position", chat_header_moves: "headerFollowsTheColumn", chat_footer_moves: "composerFollowsTheColumn",
  participants_bar_enabled: "showParticipantsBar", participants_bar_avatar_size: "avatarSize", participants_bar_avatar_shape: "barAvatarShape", participants_bar_background: "barBackground", participants_bar_gap: "barSpacing", participants_bar_align: "alignment", participants_bar_hint_position: "barHint",
  chat_widget_area_enabled: "widgetsBesideTheChat", chat_widget_center_mode: "widgetSides", chat_widget_slots: "widgetsBesideTheChat", user_bubble_color: "yourMessages", assistant_bubble_color: "characterMessages", user_bubble_color_hex: "yourBubbleColor", assistant_bubble_color_hex: "characterBubbleColor", footer_input_color_hex: "composer", message_text_color_hex: "allMessageText", plain_text_color_hex: "plainText", italic_text_color_hex: "italicActions", quoted_text_color_hex: "quotesDialogue", inline_code_text_color_hex: "inlineCode", transparent_header: "seethroughHeader", background_dim: "dim", background_blur: "blur", bubble_blur: "bubbleBlur", bubble_opacity: "bubbleOpacity", text_mode: "textOverTheImage",
} as const;

export function GroupAppearanceEditor({ draft, onChange, phone }: { draft: GroupDraft_Serialize; onChange: (draft: GroupDraft_Serialize) => void; phone: boolean }) {
  const { t } = useTranslation("groups");
  const { t: label } = useTranslation("customization");
  const { theme } = useTheme();
  const preferences = usePreferences();
  if (!preferences.ready) return preferences.error ? <FailureState error={preferences.error} onRetry={preferences.retry} /> : <LoadingState />;
  const fields = appearanceFields(draft, preferences.value);
  const look = groupAppearance(draft, preferences.value);
  const own = (key: keyof ChatLook) => appearanceFieldKeys(key).some((field) => fields.has(field));
  const reset = (keys: string[]) => onChange({ ...draft, presentation_overrides: [...fields].filter((field) => !keys.includes(field)) });
  return <div className="space-y-6">
    <p className="text-sm text-fg-3">{t("appearance.body")}</p>
    <ChatPreview look={look} />
    {fields.size > 0 && <ListGroup label={t("appearance.overrides")}>{[...fields].map((field) => <ListRow key={field} title={label(labels[field as keyof typeof labels])} description={t("appearance.own")} trailing={<Button size="sm" variant="ghost" onClick={() => reset([field])}>{t("appearance.reset")}</Button>} />)}</ListGroup>}
    <ChatSettings group look={look} phone={phone} set={(key, value) => onChange(changeGroupAppearance(draft, preferences.value, key, value, theme.colors.accent))} source={(key) => t(own(key) ? "appearance.own" : "appearance.inherited")} overridden={own} resetField={(key) => reset(appearanceFieldKeys(key))} onReset={() => onChange({ ...draft, presentation_overrides: [] })} />
  </div>;
}
