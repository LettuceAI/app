import type { i18n } from "i18next";
import { fallbackLanguage } from "@/shared/i18n";

const retainedLocales = new Set(["en", "es", "fr", "de", "ja", "pl", "pt", "no", "id", "fil", "nl", "el", "hi", "it", "vi", "ru", "ko", "tr", "zh-Hans", "zh-Hant"]);

function requestedLocales(): string[] {
  return typeof navigator === "undefined" ? [] : [navigator.language, ...(navigator.languages ?? [])];
}

/** The first of the browser's languages the app has, or null. */
export function matchRequestedLocale(requested: readonly string[] = requestedLocales()): string | null {
  for (const locale of requested) {
    if (retainedLocales.has(locale)) return locale;
    const normalized = locale.toLowerCase();
    if (["zh", "zh-cn"].includes(normalized)) return "zh-Hans";
    if (["zh-tw", "zh-hk", "zh-mo"].includes(normalized)) return "zh-Hant";
    const base = normalized.split("-")[0];
    const match = [...retainedLocales].find((candidate) => candidate.toLowerCase() === base);
    if (match) return match;
  }
  return null;
}

/**
 * The stored choice first, then a legacy choice, then the device. With `systemFirst`
 * (Android, where the system's per-app language setting is the language picker) the
 * device's languages come first.
 */
export function selectDeviceLocale(
  state: Record<string, unknown>,
  readLegacy: () => string | null,
  systemFirst = false,
): string {
  if (systemFirst) {
    const system = matchRequestedLocale();
    if (system) return system;
  }
  if (typeof state.locale === "string" && retainedLocales.has(state.locale)) return state.locale;
  try {
    const legacy = readLegacy();
    if (legacy && retainedLocales.has(legacy)) return legacy;
  } catch {}
  return matchRequestedLocale() ?? fallbackLanguage;
}

/** Switches the app language when the system's language for the app changes while it runs. */
export function followSystemLocale(localization: i18n, store: (locale: string) => Promise<void>): () => void {
  const onChange = () => {
    const next = matchRequestedLocale();
    if (!next || next === localization.language) return;
    void localization.changeLanguage(next);
    void store(next).catch(() => undefined);
  };
  window.addEventListener("languagechange", onChange);
  return () => window.removeEventListener("languagechange", onChange);
}
