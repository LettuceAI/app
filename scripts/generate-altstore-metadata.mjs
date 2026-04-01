#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

function fail(message) {
  console.error(`[altstore] ${message}`);
  process.exit(1);
}

function requireEnv(name) {
  const value = process.env[name];
  if (!value || !value.trim()) {
    fail(`Missing required environment variable: ${name}`);
  }
  return value.trim();
}

async function main() {
  const ipaPath = process.argv[2];
  const outputPath = process.argv[3];
  if (!ipaPath || !outputPath) {
    fail("Usage: node scripts/generate-altstore-metadata.mjs <ipaPath> <outputPath>");
  }

  const artifactUrl = process.env.ALTSTORE_ARTIFACT_URL || "";
  const scriptPath = fileURLToPath(import.meta.url);
  const packageJsonPath = path.resolve(path.dirname(scriptPath), "..", "package.json");
  const tauriConfigPath = path.resolve(path.dirname(scriptPath), "..", "src-tauri", "tauri.conf.json");
  const packageJson = JSON.parse(await readFile(packageJsonPath, "utf8"));
  const tauriConfig = JSON.parse(await readFile(tauriConfigPath, "utf8"));
  const version = process.env.ALTSTORE_VERSION || process.env.GITHUB_REF_NAME || packageJson.version || "0.0.0";
  const buildVersion = process.env.ALTSTORE_BUILD_VERSION || String(process.env.GITHUB_RUN_NUMBER || "1");
  const bundleIdentifier =
    process.env.ALTSTORE_BUNDLE_ID || process.env.IOS_BUNDLE_IDENTIFIER || tauriConfig.identifier || "";
  if (!bundleIdentifier) {
    fail("Missing bundle identifier. Set ALTSTORE_BUNDLE_ID or IOS_BUNDLE_IDENTIFIER.");
  }
  const appName = process.env.ALTSTORE_APP_NAME || "LettuceAI";

  const ipaBytes = await readFile(ipaPath);
  const sha256 = createHash("sha256").update(ipaBytes).digest("hex");
  const fileInfo = await stat(ipaPath);
  const fileName = path.basename(ipaPath);

  const metadata = {
    name: appName,
    bundleIdentifier,
    version,
    buildVersion,
    fileName,
    size: fileInfo.size,
    sha256,
    downloadURL: artifactUrl,
    generatedAt: new Date().toISOString(),
  };

  await writeFile(outputPath, `${JSON.stringify(metadata, null, 2)}\n`, "utf8");
  console.log(`[altstore] Metadata generated: ${outputPath}`);
}

main().catch((error) => {
  fail(error instanceof Error ? error.message : String(error));
});
