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
  } catch {}
  const requested = typeof navigator === "undefined" ? [] : [navigator.language, ...(navigator.languages ?? [])];
  for (const locale of requested) {
    if (retainedLocales.has(locale)) return locale;
    const normalized = locale.toLowerCase();
    if (["zh", "zh-cn"].includes(normalized)) return "zh-Hans";
    if (["zh-tw", "zh-hk", "zh-mo"].includes(normalized)) return "zh-Hant";
    const base = normalized.split("-")[0];
    const match = [...retainedLocales].find((candidate) => candidate.toLowerCase() === base);
    if (match) return match;
  }
  return fallbackLanguage;
}
