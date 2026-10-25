export interface ChatLook {
  fontSize: "small" | "medium" | "large" | "xlarge";
  lineSpacing: "tight" | "normal" | "relaxed";
  composerFontSize: "small" | "medium" | "large" | "xlarge";
  bubbleStyle: "bordered" | "filled" | "minimal";
  bubbleRadius: "sharp" | "rounded" | "pill";
  bubbleMaxWidth: "compact" | "normal" | "wide";
  bubblePadding: "compact" | "normal" | "spacious";
  showMessageAuthor: boolean;
  showMessageTimestamp: boolean;
  timestampFormat: "relative" | "time" | "datetime";
  messageHeaderPlacement: "inside" | "above";
  details: Detail[];
  messageInfoPlacement: "belowHeader" | "belowHeaderOutside" | "insideBubble" | "belowBubble";
  messageInfoSize: "small" | "medium" | "large";
  messageGap: "tight" | "normal" | "relaxed";
  avatarShape: "circle" | "rounded" | "hidden";
  avatarSize: "small" | "medium" | "large";
  chatColumnWidth: "narrow" | "normal" | "wide" | "xl" | "full" | "custom";
  chatColumnWidthPx: number;
  chatColumnAlign: "left" | "center" | "right";
  chatHeaderMoves: boolean;
  chatFooterMoves: boolean;
  chatWidgetAreaEnabled: boolean;
  chatWidgetCenterMode: "both" | "left" | "right";
  participantsBarEnabled: boolean;
  participantsBarAvatarSize: "small" | "medium" | "large";
  participantsBarAvatarShape: "round" | "boxed" | "rounded_box";
  participantsBarBackground: "solid" | "fading" | "transparent";
  participantsBarGap: "tight" | "normal" | "relaxed";
  participantsBarAlign: "left" | "center" | "right";
  participantsBarHintPosition: "top" | "bottom" | "hidden";
  userBubbleColor: "accent" | "info" | "secondary" | "warning" | "custom";
  assistantBubbleColor: "neutral" | "accent" | "info" | "secondary" | "custom";
  userBubbleColorHex: string | null;
  assistantBubbleColorHex: string | null;
  footerInputColorHex: string | null;
  messageTextColorHex: string | null;
  plainTextColorHex: string | null;
  italicTextColorHex: string | null;
  quotedTextColorHex: string | null;
  inlineCodeTextColorHex: string | null;
  transparentHeader: boolean;
  backgroundDim: number;
  backgroundBlur: number;
  bubbleBlur: "none" | "light" | "medium" | "heavy";
  bubbleOpacity: number;
  textMode: "auto" | "light" | "dark";
}

export type Detail = "model" | "input" | "output" | "total" | "ttft" | "tps" | "mtp";

export const DETAILS: readonly { value: Detail }[] = (
  ["model", "input", "output", "total", "ttft", "tps", "mtp"] as const
).map((value) => ({ value }));

export const CHAT_DEFAULTS: ChatLook = {
  fontSize: "medium",
  lineSpacing: "relaxed",
  composerFontSize: "medium",
  bubbleStyle: "bordered",
  bubbleRadius: "rounded",
  bubbleMaxWidth: "normal",
  bubblePadding: "normal",
  showMessageAuthor: false,
  showMessageTimestamp: false,
  timestampFormat: "relative",
  messageHeaderPlacement: "inside",
  details: [],
  messageInfoPlacement: "belowBubble",
  messageInfoSize: "small",
  messageGap: "relaxed",
  avatarShape: "circle",
  avatarSize: "medium",
  chatColumnWidth: "full",
  chatColumnWidthPx: 960,
  chatColumnAlign: "center",
  chatHeaderMoves: false,
  chatFooterMoves: false,
  chatWidgetAreaEnabled: false,
  chatWidgetCenterMode: "both",
  participantsBarEnabled: true,
  participantsBarAvatarSize: "medium",
  participantsBarAvatarShape: "round",
  participantsBarBackground: "fading",
  participantsBarGap: "normal",
  participantsBarAlign: "left",
  participantsBarHintPosition: "bottom",
  userBubbleColor: "accent",
  assistantBubbleColor: "neutral",
  userBubbleColorHex: null,
  assistantBubbleColorHex: null,
  footerInputColorHex: null,
  messageTextColorHex: null,
  plainTextColorHex: null,
  italicTextColorHex: null,
  quotedTextColorHex: null,
  inlineCodeTextColorHex: null,
  transparentHeader: false,
  backgroundDim: 0,
  backgroundBlur: 0,
  bubbleBlur: "none",
  bubbleOpacity: 35,
  textMode: "auto",
};

export const detailFields = {
  model: "showMessageModel",
  input: "showMessageInputTokens",
  output: "showMessageOutputTokens",
  total: "showMessageTotalTokens",
  ttft: "showMessageTtft",
  tps: "showMessageTokensPerSecond",
  mtp: "showMessageMtp",
} as const;

export function readChatLook(value: Record<string, unknown>): ChatLook {
  const next = { ...CHAT_DEFAULTS };
  for (const key of Object.keys(CHAT_DEFAULTS) as (keyof ChatLook)[]) {
    const chosen = value[key];
    if (chosen !== undefined && chosen !== null) Object.assign(next, { [key]: chosen });
  }
  next.details = (Object.keys(detailFields) as Detail[]).filter(
    (key) => value[detailFields[key]] === true,
  );
  if (typeof value.userBubbleColorHex === "string") next.userBubbleColor = "custom";
  if (typeof value.assistantBubbleColorHex === "string") next.assistantBubbleColor = "custom";
  return next;
}
