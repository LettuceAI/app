#!/usr/bin/env node

import { mkdir } from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";

const mode = process.argv[2];
if (!mode || !["dev", "build"].includes(mode)) {
  console.error("Usage: node scripts/run-tauri-android-local-tmp.mjs <dev|build> [...args]");
  process.exit(1);
}

const repoRoot = process.cwd();
const localTmpDir = path.join(repoRoot, ".tmp", "android-build");

async function runCommand(command, args, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
      env,
    });

    child.on("exit", (code, signal) => {
      if (signal) {
        process.kill(process.pid, signal);
        return;
      }
      if (code === 0) {
        resolve();
        return;
      }
      reject(new Error(`${command} ${args.join(" ")} exited with code ${code ?? 1}`));
    });

    child.on("error", reject);
  });
}

async function main() {
  await mkdir(localTmpDir, { recursive: true });

  const env = {
    ...process.env,
    TMPDIR: localTmpDir,
    TMP: localTmpDir,
    TEMP: localTmpDir,
  };

  console.log(`[android-local-tmp] Using TMPDIR=${localTmpDir}`);

  await runCommand("node", ["scripts/apply-android-overrides.mjs"], env);
  await runCommand("tauri", ["android", mode, ...process.argv.slice(3)], env);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
