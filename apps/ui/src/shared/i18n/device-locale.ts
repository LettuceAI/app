export const retainedLocales = new Set([
  "en",
  "es",
  "fr",
  "de",
  "ja",
  "pl",
  "pt",
  "no",
  "id",
  "fil",
  "nl",
  "el",
  "hi",
  "it",
  "vi",
  "ru",
  "ko",
  "tr",
  "zh-Hans",
  "zh-Hant",
]);

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
    const base = normalized.split("-")[0];
    const match = [...retainedLocales].find((candidate) => candidate.toLowerCase() === base);
    if (match) return match;
  }
  return null;
}
