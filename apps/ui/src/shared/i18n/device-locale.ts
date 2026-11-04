export const supportedLocales = [
  "en",
  "tr",
  "de",
  "fr",
  "es",
  "pt",
  "it",
  "pl",
  "ru",
  "ja",
  "ko",
  "zh-Hans",
  "zh-Hant",
  "id",
  "vi",
  "nl",
  "no",
  "fil",
  "el",
  "hi",
  "hu",
  "cs",
  "uk",
  "th",
] as const;

export const retainedLocales: ReadonlySet<string> = new Set(supportedLocales);

const localeAliases: Record<string, string> = { nb: "no", nn: "no", in: "id", tl: "fil" };

function requestedLocales(): string[] {
  return typeof navigator === "undefined"
    ? []
    : [navigator.language, ...(navigator.languages ?? [])];
}

/** The first of the browser's languages the app has, or null. */
export function matchRequestedLocale(
  requested: readonly string[] = requestedLocales(),
): string | null {
  for (const locale of requested) {
    if (retainedLocales.has(locale)) return locale;
    const normalized = locale.toLowerCase();
    if (["zh", "zh-cn"].includes(normalized)) return "zh-Hans";
    if (["zh-tw", "zh-hk", "zh-mo"].includes(normalized)) return "zh-Hant";
    const language = normalized.split("-")[0] ?? normalized;
    const base = localeAliases[language] ?? language;
    const match = [...retainedLocales].find((candidate) => candidate.toLowerCase() === base);
    if (match) return match;
  }
  return null;
}
