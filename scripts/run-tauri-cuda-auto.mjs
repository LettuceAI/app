#!/usr/bin/env node

import { existsSync } from "node:fs";
import { mkdir } from "node:fs/promises";
import { execFileSync, spawn } from "node:child_process";
import path from "node:path";

function detectCudaArchitectures() {
  try {
    const out = execFileSync("nvidia-smi", ["--query-gpu=compute_cap", "--format=csv,noheader"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });

    const arches = [
      ...new Set(
        out
          .split("\n")
          .map((line) => line.trim())
          .filter(Boolean)
          .map((cc) => cc.replace(".", ""))
          .filter((cc) => /^\d+$/.test(cc)),
      ),
    ];

    return arches;
  } catch {
    return [];
  }
}

const mode = process.argv[2];
if (!mode || !["dev", "build"].includes(mode)) {
  console.error("Usage: node scripts/run-tauri-cuda-auto.mjs <dev|build>");
  process.exit(1);
}

const arches = detectCudaArchitectures();
const env = { ...process.env };
const localTmpDir = path.join(process.cwd(), ".tmp", "cuda-build");

await mkdir(localTmpDir, { recursive: true });
env.TMPDIR = localTmpDir;
env.TMP = localTmpDir;
env.TEMP = localTmpDir;
console.log(`[cuda-auto] Using TMPDIR=${localTmpDir}`);

if (!env.CMAKE_CUDA_ARCHITECTURES && arches.length > 0) {
  env.CMAKE_CUDA_ARCHITECTURES = arches.join(";");
  console.log(`[cuda-auto] Detected CUDA architectures: ${env.CMAKE_CUDA_ARCHITECTURES}`);
} else if (env.CMAKE_CUDA_ARCHITECTURES) {
  console.log(
    `[cuda-auto] Using existing CMAKE_CUDA_ARCHITECTURES=${env.CMAKE_CUDA_ARCHITECTURES}`,
  );
} else {
  console.warn(
    "[cuda-auto] Could not detect NVIDIA compute capability. Falling back to llama.cpp defaults.",
  );
}

// CUDA 13.1 cudafe++ chokes on GCC 16 C++20 headers (char8_t, requires).
// Pick the newest GCC ≤ 15 available as the CUDA host compiler.
if (process.platform === "linux" && !env.CMAKE_CUDA_HOST_COMPILER && !env.CUDAHOSTCXX) {
  const candidates = ["/usr/bin/g++-15", "/usr/bin/g++-14", "/usr/bin/g++-13"];
  const hostCxx = candidates.find(existsSync);
  if (hostCxx) {
    env.CMAKE_CUDA_HOST_COMPILER = hostCxx;
    env.CUDAHOSTCXX = hostCxx;
    console.log(`[cuda-auto] CUDA host compiler pinned to ${hostCxx} (avoids GCC 16 incompatibility)`);
  } else {
    console.warn("[cuda-auto] No GCC ≤ 15 found — CUDA build may fail with GCC 16 headers. Install gcc15: sudo pacman -S gcc15");
  }
}

// Disable NCCL (multi-GPU) — not needed for single-GPU builds.
// llama-cpp-sys-2 only forwards CMAKE_* env vars to CMake, so we use the
// standard CMAKE_DISABLE_FIND_PACKAGE mechanism to prevent CMake from
// detecting NCCL. Without this, CMake enables NCCL but the build.rs never
// emits cargo:rustc-link-lib=nccl, causing undefined symbol errors.
env.CMAKE_DISABLE_FIND_PACKAGE_NCCL = env.CMAKE_DISABLE_FIND_PACKAGE_NCCL ?? "TRUE";

// Tauri builds a shared library on Linux; CUDA/C/C++ objects must be PIC-safe.
// llama-cpp-sys forwards all CMAKE_* env vars into CMake definitions.
if (process.platform === "linux") {
  env.CMAKE_POSITION_INDEPENDENT_CODE = env.CMAKE_POSITION_INDEPENDENT_CODE ?? "ON";
  env.CMAKE_C_FLAGS = `${env.CMAKE_C_FLAGS ?? ""} -fPIC`.trim();
  env.CMAKE_CXX_FLAGS = `${env.CMAKE_CXX_FLAGS ?? ""} -fPIC`.trim();
  env.CMAKE_CUDA_FLAGS = `${env.CMAKE_CUDA_FLAGS ?? ""} --compiler-options=-fPIC`.trim();
  console.log(
    `[cuda-auto] Linux PIC flags: CMAKE_POSITION_INDEPENDENT_CODE=${env.CMAKE_POSITION_INDEPENDENT_CODE}, CMAKE_CUDA_FLAGS=${env.CMAKE_CUDA_FLAGS}`,
  );
}

const child = spawn("bun", ["run", "tauri", mode, "--features", "llama-gpu-cuda"], {
  stdio: "inherit",
  env,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});
