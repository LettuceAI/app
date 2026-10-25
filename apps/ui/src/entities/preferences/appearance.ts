import type { MessageAppearance } from "@lettuceai/crisp";
import type { ChatLook } from "./chat";
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
