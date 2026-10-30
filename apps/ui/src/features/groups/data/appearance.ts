import type { GroupDraft_Serialize } from "@/api/generated/bindings";
import { CHAT_DEFAULTS, detailFields, object, readChatLook, type ChatLook } from "@/entities/preferences";

export const appearanceKey = (key: string) => key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
export const appearanceName = (key: string) => key.replace(/_([a-z])/g, (_match, letter: string) => letter.toUpperCase());

export function appearanceFields(draft: GroupDraft_Serialize, preferences: Record<string, unknown>): Set<string> {
  if (draft.presentation_overrides !== null && draft.presentation_overrides !== undefined) return new Set(draft.presentation_overrides);
  const baseline = { ...CHAT_DEFAULTS, messageInfoPlacement: "below_bubble", participantsBarBackground: "fading", chatColumnWidthPx: null, chatWidgetSlots: { left: [], right: [] }, ...object(preferences.appearanceInheritanceBaseline ?? preferences.chatAppearance) } as Record<string, unknown>;
  baseline.messageInfoPlacement = appearanceKey(String(baseline.messageInfoPlacement));
  for (const field of Object.values(detailFields)) baseline[field] ??= false;
  return new Set(Object.entries(draft.presentation).filter(([key, value]) => key !== "format_version" && JSON.stringify(value) !== JSON.stringify(baseline[appearanceName(key)])).map(([key]) => key));
}

export function groupAppearance(draft: GroupDraft_Serialize, preferences: Record<string, unknown>): ChatLook {
  const fields = appearanceFields(draft, preferences);
  const values: Record<string, unknown> = { participantsBarBackground: "fading", ...object(preferences.chatAppearance) };
  for (const [key, value] of Object.entries(draft.presentation)) if (fields.has(key)) values[appearanceName(key)] = key === "message_info_placement" && typeof value === "string" ? appearanceName(value) : value;
  return readChatLook(values);
}

export function appearanceFieldKeys(key: keyof ChatLook): string[] {
  if (key === "details") return Object.values(detailFields).map(appearanceKey);
  if (key === "userBubbleColor" || key === "assistantBubbleColor") return [appearanceKey(key), appearanceKey(`${key}Hex`)];
  return [appearanceKey(key)];
}

export function changeGroupAppearance<K extends keyof ChatLook>(draft: GroupDraft_Serialize, preferences: Record<string, unknown>, key: K, value: ChatLook[K], customColor: string): GroupDraft_Serialize {
  const fields = appearanceFields(draft, preferences);
  const presentation = { ...draft.presentation };
  if (key === "details") {
    for (const [detail, field] of Object.entries(detailFields)) Object.assign(presentation, { [appearanceKey(field)]: (value as ChatLook["details"]).includes(detail as ChatLook["details"][number]) });
  } else if (key === "userBubbleColor" || key === "assistantBubbleColor") {
    Object.assign(presentation, { [appearanceKey(key)]: value === "custom" ? (key === "userBubbleColor" ? "accent" : "neutral") : value, [appearanceKey(`${key}Hex`)]: value === "custom" ? customColor : null });
  } else {
    Object.assign(presentation, { [appearanceKey(key)]: key === "messageInfoPlacement" ? appearanceKey(String(value)) : value });
  }
  for (const field of appearanceFieldKeys(key)) fields.add(field);
  return { ...draft, presentation, presentation_overrides: [...fields].sort() };
}
