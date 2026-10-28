import { createInstance, type i18n } from "i18next";
import { initReactI18next } from "react-i18next";
import logs from "./locales/en/logs.json";
import help from "./locales/en/help.json";
import updates from "./locales/en/updates.json";
import characters from "./locales/en/characters.json";
import lorebookEditor from "./locales/en/lorebook-editor.json";
import lorebookPreview from "./locales/en/lorebook-preview.json";
import lorebookCreate from "./locales/en/lorebook-create.json";
import usage from "./locales/en/usage.json";
import companions from "./locales/en/companions.json";
import customization from "./locales/en/customization.json";
import common from "./locales/en/common.json";
import conversations from "./locales/en/conversations.json";
import home from "./locales/en/home.json";
import library from "./locales/en/library.json";
import openedFiles from "./locales/en/opened-files.json";
import onboarding from "./locales/en/onboarding.json";
import prompts from "./locales/en/prompts.json";
import personas from "./locales/en/personas.json";
import settings from "./locales/en/settings.json";
import welcomeBack from "./locales/en/welcome-back.json";

export const defaultNamespace = "common";

export const resources = {
  en: {
    help,
    updates,
    logs,
    "lorebook-create": lorebookCreate,
    "lorebook-editor": lorebookEditor,
    "lorebook-preview": lorebookPreview,
    usage,
    companions,
    customization,
    prompts,
    characters,
    common,
    conversations,
    home,
    library,
    onboarding,
    "opened-files": openedFiles,
    personas,
    settings,
    "welcome-back": welcomeBack,
  },
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
