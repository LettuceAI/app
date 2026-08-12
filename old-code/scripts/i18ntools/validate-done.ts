import { LOCALES, chunkTargets, doneFile, missingFile, readJson, samePlaceholders } from "./common";

const targets = process.argv.slice(2);
const resolvedTargets = targets.length ? targets : LOCALES.flatMap(chunkTargets);
let failed = false;

for (const target of resolvedTargets) {
  const missing = readJson<Record<string, string>>(missingFile(target), {});
  const done = readJson<Record<string, string>>(doneFile(target), {});
  const missingKeys = Object.keys(missing).filter((key) => !(key in done));
  const extraKeys = Object.keys(done).filter((key) => !(key in missing));
  const badPlaceholders = Object.entries(done)
    .filter(([key, value]) => key in missing && !samePlaceholders(missing[key], value))
    .map(([key]) => key);

  if (missingKeys.length || extraKeys.length || badPlaceholders.length) failed = true;
  console.log(
    `${target}: done=${Object.keys(done).length}/${Object.keys(missing).length} ` +
      `missing=${missingKeys.length} extra=${extraKeys.length} placeholders=${badPlaceholders.length}`,
  );
}

process.exitCode = failed ? 1 : 0;

