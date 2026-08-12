import { readdirSync } from "node:fs";
import { WORK_DIR, readJson, workFile, writeJson } from "./common";

const files = readdirSync(WORK_DIR).filter(
  (file) => file.startsWith("missing-") && file.endsWith(".json") && !/-[12]\.json$/.test(file),
);

for (const file of files) {
  const code = file.slice("missing-".length, -".json".length);
  const obj = readJson<Record<string, string>>(workFile(file), {});
  const entries = Object.entries(obj);
  const midpoint = Math.ceil(entries.length / 2);
  writeJson(workFile(`missing-${code}-1.json`), Object.fromEntries(entries.slice(0, midpoint)));
  writeJson(workFile(`missing-${code}-2.json`), Object.fromEntries(entries.slice(midpoint)));
}

console.log(`split ${files.length} locales into halves`);

