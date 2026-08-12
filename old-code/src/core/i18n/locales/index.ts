export {
  localeRegistry,
  SUPPORTED_LOCALES,
  getLocaleMetadata,
  type Locale,
  type LocaleMetadata,
  type LocaleMessages,
} from "./registry";

type ImportMetaWithGlob = ImportMeta & {
  glob: (
    pattern: string,
    options: { eager: true; import: "default" },
  ) => Record<string, string>;
};

const localeIconModules = (import.meta as ImportMetaWithGlob).glob("../localesIcons/*.svg", {
  eager: true,
  import: "default",
}) as Record<string, string>;
import type { Locale } from "./registry";

export function getLocaleIconSrc(locale: Locale): string | null {
  return localeIconModules[`../localesIcons/${locale}.svg`] ?? null;
}
