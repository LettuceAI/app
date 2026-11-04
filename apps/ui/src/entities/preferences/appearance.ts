import type { MessageAppearance } from "@lettuceai/crisp";
import type { ChatLook } from "./chat";
import { object } from "./context";

export const groupAppearanceStarted = (preferences: Record<string, unknown>) =>
  preferences.groupChatAppearance !== null &&
  typeof preferences.groupChatAppearance === "object" &&
  !Array.isArray(preferences.groupChatAppearance);

/**
 * Group chats use `groupChatAppearance` wholesale once that key exists, even as an empty
 * object (app defaults). Until then they follow normal chat; there is no field-by-field merge.
 */
export function appAppearancePreferences(preferences: Record<string, unknown>, group = false): Record<string, unknown> {
  if (!group) return { ...object(preferences.chatAppearance) };
  return {
    participantsBarBackground: "fading",
    ...object(groupAppearanceStarted(preferences) ? preferences.groupChatAppearance : preferences.chatAppearance),
  };
}

const BUBBLE_TOKENS: Record<string, string> = {
  neutral: "var(--color-fill-2)",
  accent: "var(--color-accent)",
  info: "var(--color-info)",
  secondary: "var(--color-secondary)",
  warning: "var(--color-warning)",
};
const tokenColor = (token: string, hex: string | null) =>
  token === "custom" ? (hex ?? undefined) : token === "neutral" ? undefined : BUBBLE_TOKENS[token];

export function chatAppearance(look: ChatLook): MessageAppearance {
  return {
    fontSize: look.fontSize,
    lineSpacing: look.lineSpacing,
    radius: look.bubbleRadius,
    maxWidth: look.bubbleMaxWidth,
    padding: look.bubblePadding,
    style: look.bubbleStyle,
    avatar: look.avatarShape,
    avatarSize: look.avatarSize,
    headerPlacement:
      look.messageHeaderPlacement === "above" ? "outside" : look.messageHeaderPlacement,
    showAuthor: look.showMessageAuthor,
    showTimestamp: look.showMessageTimestamp,
    gap: look.messageGap,
    ...(look.userBubbleColor !== "accent" &&
    tokenColor(look.userBubbleColor, look.userBubbleColorHex)
      ? { userColor: tokenColor(look.userBubbleColor, look.userBubbleColorHex)! }
      : {}),
    ...(tokenColor(look.assistantBubbleColor, look.assistantBubbleColorHex)
      ? { assistantColor: tokenColor(look.assistantBubbleColor, look.assistantBubbleColorHex)! }
      : {}),
    textColors: {
      ...(look.messageTextColorHex ? { message: look.messageTextColorHex } : {}),
      ...(look.plainTextColorHex ? { plain: look.plainTextColorHex } : {}),
      ...(look.italicTextColorHex ? { italic: look.italicTextColorHex } : {}),
      ...(look.quotedTextColorHex ? { quote: look.quotedTextColorHex } : {}),
      ...(look.inlineCodeTextColorHex ? { code: look.inlineCodeTextColorHex } : {}),
    },
    detailsPlacement: look.messageInfoPlacement,
    detailsSize: look.messageInfoSize,
    bubbleOpacity: look.bubbleOpacity,
    bubbleBlur: look.bubbleBlur,
  };
}
