import { fileURLToPath } from "node:url";

async function main() {
  const [command = "dev", ...args] = Bun.argv.slice(2);
  if (command !== "dev" && command !== "build") {
    throw new Error("Usage: bun scripts/cuda.ts <dev|build> [Tauri options]");
  }

  const env = { ...process.env };
  if (!args.includes("--help") && !args.includes("-h")) {
    const smi = Bun.which("nvidia-smi");
    if (!smi) throw new Error("nvidia-smi was not found. Install the NVIDIA driver and make sure nvidia-smi is on PATH.");

    const query = Bun.spawnSync([smi, "--query-gpu=compute_cap", "--format=csv,noheader"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const decode = new TextDecoder();
    if (query.exitCode !== 0) {
      const reason = decode.decode(query.stderr).trim();
      throw new Error(`Could not detect CUDA architectures: ${reason || `nvidia-smi exited with code ${query.exitCode}`}`);
    }

    const capabilities = decode.decode(query.stdout).split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
    if (capabilities.length === 0) throw new Error("nvidia-smi reported no NVIDIA GPUs.");

    const architectures = [...new Set(capabilities.map((capability) => {
      const match = /^([1-9]\d*)\.(\d)$/.exec(capability);
      if (!match) throw new Error(`Unsupported CUDA compute capability reported by nvidia-smi: ${capability}`);
      return `${match[1]}${match[2]}-real`;
    }))].join(";");

    env.CMAKE_CUDA_ARCHITECTURES = architectures;
    console.log(`CUDA architectures: ${architectures}`);
  }

  const child = Bun.spawn([process.execPath, "run", "tauri", command, "--features", "cuda", ...args], {
    cwd: fileURLToPath(new URL("../", import.meta.url)),
    env,
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  });
  process.exit(await child.exited);
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
