#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { access, copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { constants as fsConstants } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ORT_VERSION = "1.22.0";
const ARCHIVE_URL = `https://download.onnxruntime.ai/pod-archive-onnxruntime-c-${ORT_VERSION}.zip`;
const SUPPORTED_SLICES = new Set(["ios-arm64", "ios-arm64_x86_64-simulator"]);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");
const installRoot = path.join(repoRoot, "src-tauri", "onnxruntime-ios");
const archivePath = path.join(installRoot, `.pod-archive-onnxruntime-c-${ORT_VERSION}.zip`);

function fail(message) {
  console.error(`[ios-ort] ${message}`);
  process.exit(1);
}

function run(command, args) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    cwd: repoRoot,
  });

  if (result.error) {
    fail(`Failed to run ${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`${command} exited with code ${result.status ?? 1}`);
  }
}

async function exists(pathname) {
  try {
    await access(pathname, fsConstants.F_OK);
    return true;
  } catch {
    return false;
  }
}

async function ensureArchive() {
  if (await exists(archivePath)) {
    return;
  }

  await mkdir(installRoot, { recursive: true });
  console.log(`[ios-ort] Downloading ${ARCHIVE_URL}`);
  run("curl", ["-L", "--fail", "--output", archivePath, ARCHIVE_URL]);
}

async function installSlice(slice) {
  if (!SUPPORTED_SLICES.has(slice)) {
    fail(`Unsupported iOS ONNX Runtime slice: ${slice}`);
  }

  const sliceRoot = path.join(installRoot, slice);
  const versionFile = path.join(sliceRoot, ".ort-version");
  const libPath = path.join(sliceRoot, "libonnxruntime.a");

  if ((await exists(libPath)) && (await exists(versionFile))) {
    const currentVersion = (await readFile(versionFile, "utf8")).trim();
    if (currentVersion === ORT_VERSION) {
      console.log(`[ios-ort] Using existing ${slice} install at ${sliceRoot}`);
      return sliceRoot;
    }
  }

  await ensureArchive();
  await mkdir(sliceRoot, { recursive: true });

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "onnxruntime-ios-"));

  try {
    run("unzip", ["-oq", archivePath, "-d", tempRoot]);

    const frameworkBinary = path.join(
      tempRoot,
      "onnxruntime.xcframework",
      slice,
      "onnxruntime.framework",
      "onnxruntime",
    );

    if (!(await exists(frameworkBinary))) {
      fail(`Expected ONNX Runtime binary is missing from archive for slice ${slice}`);
    }

    await copyFile(frameworkBinary, libPath);
    await writeFile(versionFile, `${ORT_VERSION}\n`, "utf8");
    console.log(`[ios-ort] Installed ${slice} to ${sliceRoot}`);
    return sliceRoot;
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
}

async function main() {
  const slice = process.argv[2];
  if (!slice) {
    fail("Usage: node scripts/install-onnxruntime-ios.mjs <ios-arm64|ios-arm64_x86_64-simulator>");
  }

  await installSlice(slice);
}

main().catch((error) => {
  fail(error instanceof Error ? error.message : String(error));
});
