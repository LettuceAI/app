import type { i18n } from "i18next";
import { fallbackLanguage } from "@/shared/i18n";

import { matchRequestedLocale, retainedLocales } from "@/shared/i18n/device-locale";

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
  if (state.localeChoice === "system") return matchRequestedLocale() ?? fallbackLanguage;
  if (typeof state.localeChoice === "string" && retainedLocales.has(state.localeChoice))
    return state.localeChoice;
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
export function followSystemLocale(
  localization: i18n,
  store: (locale: string) => Promise<void>,
  shouldFollow: () => boolean = () => true,
): () => void {
  const onChange = () => {
    if (!shouldFollow()) return;
    const next = matchRequestedLocale();
    if (!next || next === localization.language) return;
    void localization.changeLanguage(next);
    void store(next).catch(() => undefined);
  };
  window.addEventListener("languagechange", onChange);
  return () => window.removeEventListener("languagechange", onChange);
}
