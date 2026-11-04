#!/usr/bin/env bash
set -euo pipefail

accelerator=${1:?Expected cpu, vulkan or cuda}
case "$accelerator" in cpu|vulkan|cuda) ;; *) echo "Unsupported accelerator: $accelerator" >&2; exit 1 ;; esac

apt-get update
apt-get install -y --no-install-recommends \
  build-essential clang libclang-dev llvm-dev cmake ninja-build pkg-config \
  curl git ca-certificates unzip python3 libasound2-dev libwebkit2gtk-4.1-dev \
  libgtk-3-dev libsoup-3.0-dev libdbus-1-dev libssl-dev libwayland-dev \
  libx11-dev librsvg2-dev patchelf
if [[ "$accelerator" == vulkan ]]; then
  apt-get install -y --no-install-recommends libvulkan-dev glslc spirv-headers
elif [[ "$accelerator" == cuda ]]; then
  major=$(cat /tmp/lettuceai-build/cuda-gcc-version)
  [[ "$major" =~ ^[0-9]+$ ]]
  apt-get install -y --no-install-recommends "gcc-$major" "g++-$major"
fi
rm -rf /var/lib/apt/lists/*
