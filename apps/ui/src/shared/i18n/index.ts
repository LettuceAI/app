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
import performance from "./locales/en/performance.json";
import companions from "./locales/en/companions.json";
import customization from "./locales/en/customization.json";
import common from "./locales/en/common.json";
import conversations from "./locales/en/conversations.json";
import creator from "./locales/en/creator.json";
import groups from "./locales/en/groups.json";
import home from "./locales/en/home.json";
import imageInput from "./locales/en/image-input.json";
import library from "./locales/en/library.json";
import openedFiles from "./locales/en/opened-files.json";
import onboarding from "./locales/en/onboarding.json";
import prompts from "./locales/en/prompts.json";
import personas from "./locales/en/personas.json";
import playground from "./locales/en/playground.json";
import settings from "./locales/en/settings.json";
import sync from "./locales/en/sync.json";
import welcomeBack from "./locales/en/welcome-back.json";

export const defaultNamespace = "common";

const english = {
  en: {
    creator,
    help,
    updates,
    logs,
    "lorebook-create": lorebookCreate,
    "lorebook-editor": lorebookEditor,
    "lorebook-preview": lorebookPreview,
    usage,
    performance,
    companions,
    customization,
    prompts,
    characters,
    common,
    conversations,
    home,
    "image-input": imageInput,
    groups,
    library,
    onboarding,
    "opened-files": openedFiles,
    personas,
    playground,
    settings,
    sync,
    "welcome-back": welcomeBack,
  },
} as const;

type Namespaces = Record<string, Record<string, unknown>>;

const bundles = import.meta.glob<Record<string, unknown>>("./locales/*/*.json", {
  eager: true,
  import: "default",
});

const translated: Record<string, Namespaces> = {};
for (const [path, bundle] of Object.entries(bundles)) {
  const [, language, file] = path.match(/\.\/locales\/([^/]+)\/([^/]+)\.json$/) ?? [];
  if (!language || !file || language === "en") continue;
  (translated[language] ??= {})[file] = bundle;
}

export const resources: typeof english & Record<string, Namespaces> = { ...translated, ...english };

export type Language = string;

export const fallbackLanguage = "en";

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
