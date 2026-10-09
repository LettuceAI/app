import { fallbackLanguage } from "@/shared/i18n";

const retainedLocales = new Set(["en", "es", "fr", "de", "ja", "pl", "pt", "no", "id", "fil", "nl", "el", "hi", "it", "vi", "ru", "ko", "tr", "zh-Hans", "zh-Hant"]);

export function selectDeviceLocale(
  state: Record<string, unknown>,
  readLegacy: () => string | null,
): string {
  if (typeof state.locale === "string" && retainedLocales.has(state.locale)) return state.locale;
  try {
    const legacy = readLegacy();
    if (legacy && retainedLocales.has(legacy)) return legacy;
  } catch {
    return fallbackLanguage;
  }
  return fallbackLanguage;
}
