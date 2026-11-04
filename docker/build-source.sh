#!/usr/bin/env bash
set -euo pipefail

accelerator=${1:?Expected cpu, vulkan or cuda}
features=()
case "$accelerator" in
  cpu) ;;
  vulkan) features=(--features vulkan) ;;
  cuda)
    major=$(cat scripts/cuda-gcc-version)
    [[ "$major" =~ ^[0-9]+$ ]]
    export CC="/usr/bin/gcc-$major" CXX="/usr/bin/g++-$major"
    export CUDAHOSTCXX="$CXX" NVCC_CCBIN="$CXX"
    features=(--features cuda)
    ;;
  *) echo "Unsupported accelerator: $accelerator" >&2; exit 1 ;;
esac

bun run setup
bun run tauri build --ci --no-bundle "${features[@]}" -- --locked
binary=$(python3 -c 'import tomllib; from pathlib import Path; print(tomllib.loads(Path("apps/tauri/Cargo.toml").read_text())["package"]["default-run"])')
install -Dm755 "target/release/$binary" /out/lettuceai
bun -e 'await Bun.write("/out/version", JSON.parse(await Bun.file("package.json").text()).version)'
