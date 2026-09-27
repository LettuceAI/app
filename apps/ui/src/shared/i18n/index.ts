import { createInstance, type i18n } from "i18next";
import { initReactI18next } from "react-i18next";
import common from "./locales/en/common.json";

export const defaultNamespace = "common";

export const resources = {
  en: { common },
} as const;

export type Language = keyof typeof resources;

export const fallbackLanguage: Language = "en";

/** One i18next instance per app (or per test); features add their namespaces to `resources`. */
export async function createI18n(language: string = fallbackLanguage): Promise<i18n> {
  const instance = createInstance();
  await instance.use(initReactI18next).init({
    resources,
    lng: language,
    fallbackLng: fallbackLanguage,
    ns: Object.keys(resources[fallbackLanguage]),
    defaultNS: defaultNamespace,
    interpolation: { escapeValue: false },
    returnNull: false,
  });
  return instance;
}
