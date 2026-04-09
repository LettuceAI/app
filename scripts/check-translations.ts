/**
 * Compare all locale files against en.ts to find missing translation keys.
 *
 * Usage: npx tsx scripts/check-translations.ts
 */

import { enMessages, enMetadata } from "../src/core/i18n/locales/en";
import { deMessages, deMetadata } from "../src/core/i18n/locales/de";
import { elMessages, elMetadata } from "../src/core/i18n/locales/el";
import { esMessages, esMetadata } from "../src/core/i18n/locales/es";
import { filMessages, filMetadata } from "../src/core/i18n/locales/fil";
import { frMessages, frMetadata } from "../src/core/i18n/locales/fr";
import { hiMessages, hiMetadata } from "../src/core/i18n/locales/hi";
import { idMessages, idMetadata } from "../src/core/i18n/locales/id";
import { itMessages, itMetadata } from "../src/core/i18n/locales/it";
import { jaMessages, jaMetadata } from "../src/core/i18n/locales/ja";
import { koMessages, koMetadata } from "../src/core/i18n/locales/ko";
import { nlMessages, nlMetadata } from "../src/core/i18n/locales/nl";
import { noMessages, noMetadata } from "../src/core/i18n/locales/no";
import { plMessages, plMetadata } from "../src/core/i18n/locales/pl";
import { ptMessages, ptMetadata } from "../src/core/i18n/locales/pt";
import { ruMessages, ruMetadata } from "../src/core/i18n/locales/ru";
import { viMessages, viMetadata } from "../src/core/i18n/locales/vi";
import { zhHantMessages, zhHantMetadata } from "../src/core/i18n/locales/zh-Hant";

const localeRegistry = {
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
  "zh-Hant": { messages: zhHantMessages, metadata: zhHantMetadata },
} as const;

function flattenKeys(obj: Record<string, unknown>, prefix = ""): string[] {
  const keys: string[] = [];
  for (const [key, value] of Object.entries(obj)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === "object" && !Array.isArray(value)) {
      keys.push(...flattenKeys(value as Record<string, unknown>, path));
    } else {
      keys.push(path);
    }
  }
  return keys;
}

const enKeys = new Set(flattenKeys(localeRegistry.en.messages));

console.log(`\n📋 English (en.ts): ${enKeys.size} keys\n`);
console.log("─".repeat(60));

let totalMissing = 0;
let totalExtra = 0;

for (const [locale, { messages, metadata }] of Object.entries(localeRegistry)) {
  if (locale === "en") continue;

  const localeKeys = new Set(flattenKeys(messages as Record<string, unknown>));

  const missing = [...enKeys].filter((k) => !localeKeys.has(k));
  const extra = [...localeKeys].filter((k) => !enKeys.has(k));

  const pct = Math.round(((enKeys.size - missing.length) / enKeys.size) * 100);

  console.log(
    `\n${metadata.label} (${locale}) — ${pct}% complete (${enKeys.size - missing.length}/${enKeys.size})`,
  );

  if (missing.length > 0) {
    totalMissing += missing.length;

    // Group by top-level section
    const grouped: Record<string, string[]> = {};
    for (const key of missing) {
      const section = key.split(".")[0];
      (grouped[section] ??= []).push(key);
    }

    for (const [section, keys] of Object.entries(grouped).sort(
      (a, b) => b[1].length - a[1].length,
    )) {
      console.log(`  ❌ ${section} (${keys.length} missing)`);
      for (const key of keys.slice(0, 5)) {
        console.log(`     - ${key}`);
      }
      if (keys.length > 5) {
        console.log(`     ... and ${keys.length - 5} more`);
      }
    }
  }

  if (extra.length > 0) {
    totalExtra += extra.length;
    console.log(`  ⚠️  ${extra.length} extra keys not in en.ts:`);
    for (const key of extra.slice(0, 3)) {
      console.log(`     + ${key}`);
    }
    if (extra.length > 3) {
      console.log(`     ... and ${extra.length - 3} more`);
    }
  }

  if (missing.length === 0 && extra.length === 0) {
    console.log("  ✅ Fully translated");
  }
}

console.log("\n" + "─".repeat(60));
console.log(
  `\nTotal: ${totalMissing} missing keys, ${totalExtra} extra keys across ${Object.keys(localeRegistry).length - 1} locales\n`,
);
