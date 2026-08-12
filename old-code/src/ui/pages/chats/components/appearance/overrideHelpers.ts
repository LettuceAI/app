import type {
  ChatAppearanceOverride,
  ChatAppearanceSettings,
} from "../../../../../core/storage/schemas";
import { normalizeHexColor } from "../../../../../core/utils/imageAnalysis";

type AppearanceKey = keyof ChatAppearanceSettings;

export function normalizeOverride(override: ChatAppearanceOverride): ChatAppearanceOverride {
  const normalized = { ...override } as ChatAppearanceOverride;
  normalized.userBubbleColorHex = normalizeHexColor(override.userBubbleColorHex);
  normalized.assistantBubbleColorHex = normalizeHexColor(override.assistantBubbleColorHex);
  normalized.messageTextColorHex = normalizeHexColor(override.messageTextColorHex);
  normalized.plainTextColorHex = normalizeHexColor(override.plainTextColorHex);
  normalized.italicTextColorHex = normalizeHexColor(override.italicTextColorHex);
  normalized.quotedTextColorHex = normalizeHexColor(override.quotedTextColorHex);
  normalized.inlineCodeTextColorHex = normalizeHexColor(override.inlineCodeTextColorHex);
  return Object.fromEntries(
    Object.entries(normalized)
      .filter(([_, value]) => value !== undefined)
      .sort(([a], [b]) => a.localeCompare(b)),
  ) as ChatAppearanceOverride;
}

export function deriveOverrideFromSettings(
  global: ChatAppearanceSettings,
  effective: ChatAppearanceSettings,
): ChatAppearanceOverride {
  const next: Record<string, unknown> = {};
  for (const key of Object.keys(effective) as AppearanceKey[]) {
    if (JSON.stringify(effective[key]) === JSON.stringify(global[key])) continue;
    next[key] = effective[key];
  }
  return normalizeOverride(next as ChatAppearanceOverride);
}

export function areOverridesEqual(
  a: ChatAppearanceOverride,
  b: ChatAppearanceOverride,
): boolean {
  return JSON.stringify(normalizeOverride(a)) === JSON.stringify(normalizeOverride(b));
}
