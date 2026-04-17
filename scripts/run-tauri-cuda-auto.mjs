#!/usr/bin/env node

import { execFileSync, spawn } from "node:child_process";

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
  // Use shell only on Windows for PATH resolution; avoid shell on other platforms for safety and portability
  shell: process.platform === "win32",
  env,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});
