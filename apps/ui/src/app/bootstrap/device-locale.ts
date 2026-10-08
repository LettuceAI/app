import { fallbackLanguage } from "@/shared/i18n";

export function selectDeviceLocale(
  state: Record<string, unknown>,
  readLegacy: () => string | null,
): string {
  if (typeof state.locale === "string" && state.locale.trim()) return state.locale;
  try {
    const legacy = readLegacy();
    if (legacy?.trim()) return legacy;
  } catch {
    return fallbackLanguage;
  }
  return fallbackLanguage;
}
