import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

async function main() {
  const [command = "dev", ...args] = Bun.argv.slice(2);
  if (command !== "dev" && command !== "build") {
    throw new Error("Usage: bun scripts/cuda.ts <dev|build> [Tauri options]");
  }

  const env = { ...process.env };
  if (!args.includes("--help") && !args.includes("-h")) {
    if (process.platform === "linux") {
      const major = readFileSync(new URL("./cuda-gcc-version", import.meta.url), "utf8").trim();
      if (!/^\d+$/.test(major)) throw new Error("Invalid pinned CUDA GCC version in scripts/cuda-gcc-version.");
      const cc = Bun.which(`gcc-${major}`);
      const cxx = Bun.which(`g++-${major}`);
      if (!cc || !cxx) throw new Error(`Linux CUDA builds require gcc-${major} and g++-${major}. Install both compilers and make sure they are on PATH.`);
      for (const compiler of [cc, cxx]) {
        const result = Bun.spawnSync([compiler, "-dumpversion"], { stdout: "pipe", stderr: "pipe" });
        if (result.exitCode !== 0 || new TextDecoder().decode(result.stdout).trim().split(".")[0] !== major) {
          throw new Error(`Expected GCC ${major}, but ${compiler} did not report that version.`);
        }
      }
      Object.assign(env, { CC: cc, CXX: cxx, CUDAHOSTCXX: cxx, NVCC_CCBIN: cxx });
      console.log(`CUDA host compiler: GCC ${major} (${cxx})`);
    }

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
