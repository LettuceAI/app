/**
 * Compare all locale files against en.ts to find missing translation keys.
 *
 * Usage: npx tsx scripts/check-translations.ts
 */

import { localeRegistry } from "../src/core/i18n/locales/registry";
import {
  getEnglishTranslationKeyCount,
  getLocaleTranslationReport,
} from "../src/core/i18n/translationStatus";

console.log(`\n📋 English (en.ts): ${getEnglishTranslationKeyCount()} keys\n`);
console.log("─".repeat(60));

let totalMissing = 0;
let totalExtra = 0;

for (const [locale, { messages, metadata }] of Object.entries(localeRegistry)) {
  if (locale === "en") continue;

  void messages;
  const report = getLocaleTranslationReport(locale as keyof typeof localeRegistry);

  console.log(
    `\n${metadata.label} (${locale}) — ${report.percent}% complete (${report.translatedKeys}/${report.totalKeys})`,
  );

  if (report.missing.length > 0) {
    totalMissing += report.missingKeys;

    // Group by top-level section
    const grouped: Record<string, string[]> = {};
    for (const key of report.missing) {
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

  if (report.extra.length > 0) {
    totalExtra += report.extraKeys;
    console.log(`  ⚠️  ${report.extraKeys} extra keys not in en.ts:`);
    for (const key of report.extra.slice(0, 3)) {
      console.log(`     + ${key}`);
    }
    if (report.extra.length > 3) {
      console.log(`     ... and ${report.extra.length - 3} more`);
    }
  }

  if (report.missingKeys === 0 && report.extraKeys === 0) {
    console.log("  ✅ Fully translated");
  }
}

console.log("\n" + "─".repeat(60));
console.log(
  `\nTotal: ${totalMissing} missing keys, ${totalExtra} extra keys across ${Object.keys(localeRegistry).length - 1} locales\n`,
);
