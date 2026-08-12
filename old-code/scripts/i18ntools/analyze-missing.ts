import { LOCALES, englishFlatMessages, localeFlatMessages, missingFile, writeJson } from "./common";

const english = englishFlatMessages();
const keys = Object.keys(english);

console.log("en total leaf keys:", keys.length);
console.log("missing per locale:");

for (const code of LOCALES) {
  const locale = localeFlatMessages(code);
  const missing: Record<string, string> = {};
  for (const key of keys) {
    if (!(key in locale)) missing[key] = english[key];
  }
  writeJson(missingFile(code), missing);
  console.log(`  ${code}: ${Object.keys(missing).length}`);
}

