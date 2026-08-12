import { localeRegistry, type Locale } from "./locales/registry";

type MessageTree = Record<string, unknown>;

function isBranch(value: unknown): value is MessageTree {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

interface WalkResult {
  totalKeys: number;
  translatedKeys: number;
  missing: string[];
  extra: string[];
}

function collectExtraKeys(
  localeNode: MessageTree,
  englishNode: MessageTree,
  prefix: string,
  extra: string[],
): void {
  for (const [key, localeValue] of Object.entries(localeNode)) {
    const path = prefix ? `${prefix}.${key}` : key;
    const englishValue = englishNode[key];

    if (englishValue === undefined) {
      if (isBranch(localeValue)) {
        collectLeafPaths(localeValue, path, extra);
      } else {
        extra.push(path);
      }
    } else if (isBranch(localeValue) && isBranch(englishValue)) {
      collectExtraKeys(localeValue, englishValue, path, extra);
    }
  }
}

function collectLeafPaths(node: MessageTree, prefix: string, keys: string[]): void {
  for (const [key, value] of Object.entries(node)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (isBranch(value)) {
      collectLeafPaths(value, path, keys);
    } else {
      keys.push(path);
    }
  }
}

function walkEnglishAgainstLocale(
  englishNode: MessageTree,
  localeNode: MessageTree | undefined,
  prefix: string,
): WalkResult {
  const result: WalkResult = {
    totalKeys: 0,
    translatedKeys: 0,
    missing: [],
    extra: [],
  };

  for (const [key, englishValue] of Object.entries(englishNode)) {
    const path = prefix ? `${prefix}.${key}` : key;
    const localeValue = localeNode?.[key];

    if (isBranch(englishValue)) {
      if (isBranch(localeValue)) {
        const nested = walkEnglishAgainstLocale(englishValue, localeValue, path);
        result.totalKeys += nested.totalKeys;
        result.translatedKeys += nested.translatedKeys;
        result.missing.push(...nested.missing);
        result.extra.push(...nested.extra);
      } else {
        const missingLeaves: string[] = [];
        collectLeafPaths(englishValue, path, missingLeaves);
        result.totalKeys += missingLeaves.length;
        result.missing.push(...missingLeaves);
        if (localeValue !== undefined) {
          result.extra.push(path);
        }
      }
      continue;
    }

    result.totalKeys += 1;
    if (localeValue === undefined || isBranch(localeValue)) {
      result.missing.push(path);
      if (isBranch(localeValue)) {
        result.extra.push(path);
      }
    } else {
      result.translatedKeys += 1;
    }
  }

  if (localeNode) {
    collectExtraKeys(localeNode, englishNode, prefix, result.extra);
  }

  return result;
}

const englishMessages = localeRegistry.en.messages as MessageTree;
const englishStats = walkEnglishAgainstLocale(englishMessages, englishMessages, "");

export interface LocaleTranslationCompletion {
  locale: Locale;
  totalKeys: number;
  translatedKeys: number;
  missingKeys: number;
  extraKeys: number;
  percent: number;
}

export interface LocaleTranslationReport extends LocaleTranslationCompletion {
  missing: string[];
  extra: string[];
}

export function getEnglishTranslationKeyCount(): number {
  return englishStats.totalKeys;
}

export function getLocaleTranslationReport(locale: Locale): LocaleTranslationReport {
  const localeMessages = localeRegistry[locale].messages as MessageTree;
  const result = walkEnglishAgainstLocale(englishMessages, localeMessages, "");
  const missingKeys = result.missing.length;
  const extraKeys = result.extra.length;

  return {
    locale,
    totalKeys: result.totalKeys,
    translatedKeys: result.translatedKeys,
    missingKeys,
    extraKeys,
    percent: Math.round((result.translatedKeys / result.totalKeys) * 100),
    missing: result.missing,
    extra: result.extra,
  };
}

export function getLocaleTranslationCompletion(locale: Locale): LocaleTranslationCompletion {
  const { missing, extra, ...completion } = getLocaleTranslationReport(locale);
  void missing;
  void extra;
  return completion;
}
