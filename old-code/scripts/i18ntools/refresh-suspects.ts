import {
  LOCALES,
  chunkTargets,
  doneFile,
  isSuspiciousUnchanged,
  missingFile,
  readJson,
  suspectFile,
  writeJson,
} from "./common";

const targets = process.argv.slice(2);
const resolvedTargets = targets.length ? targets : LOCALES.flatMap(chunkTargets);

for (const target of resolvedTargets) {
  const missing = readJson<Record<string, string>>(missingFile(target), {});
  const done = readJson<Record<string, string>>(doneFile(target), {});
  const suspects: Record<string, string> = {};

  for (const [key, source] of Object.entries(missing)) {
    const translated = done[key];
    if (typeof translated === "string" && isSuspiciousUnchanged(source, translated)) {
      suspects[key] = translated;
    }
  }

  writeJson(suspectFile(target), suspects);
  console.log(`${target}: suspects=${Object.keys(suspects).length}`);
}

