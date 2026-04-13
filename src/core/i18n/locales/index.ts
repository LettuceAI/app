import type { DeepPartialMessageTree } from "../types";
import { enMessages, enMetadata, type LocaleMessages } from "./en";
import { deMessages, deMetadata } from "./de";
import { elMessages, elMetadata } from "./el";
import { esMessages, esMetadata } from "./es";
import { filMessages, filMetadata } from "./fil";
import { frMessages, frMetadata } from "./fr";
import { hiMessages, hiMetadata } from "./hi";
import { idMessages, idMetadata } from "./id";
import { itMessages, itMetadata } from "./it";
import { jaMessages, jaMetadata } from "./ja";
import { koMessages, koMetadata } from "./ko";
import { nlMessages, nlMetadata } from "./nl";
import { noMessages, noMetadata } from "./no";
import { plMessages, plMetadata } from "./pl";
import { ptMessages, ptMetadata } from "./pt";
import { ruMessages, ruMetadata } from "./ru";
import { trMessages, trMetadata } from "./tr";
import { viMessages, viMetadata } from "./vi";
import { zhHansMessages, zhHansMetadata } from "./zh-Hans";
import { zhHantMessages, zhHantMetadata } from "./zh-Hant";

export interface LocaleMetadata {
  name: string;
  label: string;
}

export const localeRegistry = {
  en: { messages: enMessages, metadata: enMetadata },
  es: { messages: esMessages, metadata: esMetadata },
  fr: { messages: frMessages, metadata: frMetadata },
  de: { messages: deMessages, metadata: deMetadata },
  ja: { messages: jaMessages, metadata: jaMetadata },
  pl: { messages: plMessages, metadata: plMetadata },
  pt: { messages: ptMessages, metadata: ptMetadata },
  no: { messages: noMessages, metadata: noMetadata },
  id: { messages: idMessages, metadata: idMetadata },
  fil: { messages: filMessages, metadata: filMetadata },
  nl: { messages: nlMessages, metadata: nlMetadata },
  el: { messages: elMessages, metadata: elMetadata },
  hi: { messages: hiMessages, metadata: hiMetadata },
  it: { messages: itMessages, metadata: itMetadata },
  vi: { messages: viMessages, metadata: viMetadata },
  ru: { messages: ruMessages, metadata: ruMetadata },
  ko: { messages: koMessages, metadata: koMetadata },
  tr: { messages: trMessages, metadata: trMetadata },
  "zh-Hans": { messages: zhHansMessages, metadata: zhHansMetadata },
  "zh-Hant": { messages: zhHantMessages, metadata: zhHantMetadata },
} as const satisfies Record<
  string,
  { messages: DeepPartialMessageTree<LocaleMessages>; metadata: LocaleMetadata }
>;

export type Locale = keyof typeof localeRegistry;

const localeIconModules = import.meta.glob("../localesIcons/*.svg", {
  eager: true,
  import: "default",
}) as Record<string, string>;

export const SUPPORTED_LOCALES: readonly Locale[] = [
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
];

export function getLocaleIconSrc(locale: Locale): string | null {
  return localeIconModules[`../localesIcons/${locale}.svg`] ?? null;
}

export function getLocaleMetadata(locale: Locale): LocaleMetadata {
  return localeRegistry[locale].metadata;
}

export type { LocaleMessages };
