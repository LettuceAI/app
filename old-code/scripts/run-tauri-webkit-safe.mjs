#!/usr/bin/env node

import { spawn } from "node:child_process";

const mode = process.argv[2];
if (!mode || !["dev", "build"].includes(mode)) {
    console.error("Usage: node scripts/run-tauri-webkit-safe.mjs <dev|build>");
    process.exit(1);
}

const extraArgs = process.argv.slice(3);
const env = {
    ...process.env,
    WEBKIT_DISABLE_COMPOSITING_MODE: "1",
    WEBKIT_DISABLE_GPU_PROCESS: "1",
};

console.log(
    `[webkit-safe] WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_GPU_PROCESS=1`
);

const child = spawn("bun", ["run", "tauri", mode, ...extraArgs], {
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
