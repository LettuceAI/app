import { LOCALES, chunkTargets, doneFile, missingFile, readJson } from "./common";

let remaining = 0;
const complete: string[] = [];

for (const code of LOCALES) {
  const parts: string[] = [];
  let allComplete = true;

  for (const target of chunkTargets(code)) {
    const missing = readJson<Record<string, string>>(missingFile(target), {});
    const done = readJson<Record<string, string>>(doneFile(target), {});
    const missingCount = Object.keys(missing).length;
    const doneCount = Object.keys(done).length;
    const status = doneCount === missingCount ? "done" : doneCount > 0 ? "partial" : "todo";

    if (status !== "done") {
      allComplete = false;
      remaining += Math.max(0, missingCount - doneCount);
      parts.push(`${target.split("-").at(-1)}:${status} ${doneCount}/${missingCount}`);
    }
  }

  if (allComplete) complete.push(code);
  else console.log(`${code} ${parts.join(" | ")}`);
}

console.log("complete_locales", complete.join(","));
console.log("checkpoint_remaining_keys", remaining);

