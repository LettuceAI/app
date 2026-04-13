import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import { localeRegistry, SUPPORTED_LOCALES, type Locale, type LocaleMessages } from "./locales";
import type { DotPath, TranslateParams } from "./types";

const STORAGE_KEY = "app-locale";

export type TranslationKey = DotPath<LocaleMessages>;
export type { TranslateParams, Locale };
export { SUPPORTED_LOCALES };

function isSupportedLocale(value: string | null): value is Locale {
  return !!value && (SUPPORTED_LOCALES as readonly string[]).includes(value);
}

function getByPath(tree: unknown, key: string): string | undefined {
  const segments = key.split(".");
  let current: unknown = tree;

  for (const segment of segments) {
    if (!current || typeof current !== "object") return undefined;
    current = (current as Record<string, unknown>)[segment];
  }

  return typeof current === "string" ? current : undefined;
}

function interpolate(template: string, params?: TranslateParams): string {
  if (!params) return template;
  return template.replace(/\{\{(\w+)\}\}/g, (_match, name: string) =>
    params[name] !== undefined ? String(params[name]) : "",
  );
}

function hasUsableTranslation(value: string | undefined): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function detectSupportedLocale(value: string | null | undefined): Locale | null {
  if (!value) return null;
  if (isSupportedLocale(value)) return value;

  const normalized = value.toLowerCase();
  if (normalized === "zh-cn" || normalized === "zh") {
    return "zh-Hans";
  }
  if (normalized === "zh-tw" || normalized === "zh-hk" || normalized === "zh-mo") {
    return "zh-Hant";
  }

  const base = normalized.split("-")[0];
  return SUPPORTED_LOCALES.find((locale) => locale.toLowerCase() === base) ?? null;
}

function detectInitialLocale(): Locale {
  if (typeof window === "undefined") return "en";

  const saved = window.localStorage.getItem(STORAGE_KEY);
  if (isSupportedLocale(saved)) return saved;

  const requestedLocales = [window.navigator.language, ...(window.navigator.languages ?? [])];
  const detected =
    requestedLocales
      .map((locale) => detectSupportedLocale(locale))
      .find((locale): locale is Locale => locale !== null) ?? "en";

  // Persist the detected locale so it sticks across reloads (first boot)
  window.localStorage.setItem(STORAGE_KEY, detected);
  return detected;
}

function translateWithLocale(
  locale: Locale,
  key: TranslationKey,
  params?: TranslateParams,
): string {
  const localized = getByPath(localeRegistry[locale].messages, key);
  const fallback = getByPath(localeRegistry.en.messages, key);
  const resolved = hasUsableTranslation(localized)
    ? localized
    : hasUsableTranslation(fallback)
      ? fallback
      : key;
  return interpolate(resolved, params);
}

interface I18nContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: TranslationKey, params?: TranslateParams) => string;
  scope: (namespace: string) => (key: string, params?: TranslateParams) => string;
  tLocale: (locale: Locale) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => detectInitialLocale());

  useEffect(() => {
    if (typeof document !== "undefined") {
      document.documentElement.lang = locale;
    }
  }, [locale]);

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(STORAGE_KEY, next);
    }
  }, []);

  const t = useCallback(
    (key: TranslationKey, params?: TranslateParams) => translateWithLocale(locale, key, params),
    [locale],
  );

  const scope = useCallback(
    (namespace: string) => (key: string, params?: TranslateParams) =>
      t(`${namespace}.${key}` as TranslationKey, params),
    [t],
  );

  const tLocale = useCallback((nextLocale: Locale) => {
    const { label, name } = localeRegistry[nextLocale].metadata;
    return `${label} (${name})`;
  }, []);

  const value = useMemo(
    () => ({
      locale,
      setLocale,
      t,
      scope,
      tLocale,
    }),
    [locale, setLocale, t, scope, tLocale],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const context = useContext(I18nContext);
  if (!context) {
    throw new Error("useI18n must be used within an I18nProvider");
  }
  return context;
}
