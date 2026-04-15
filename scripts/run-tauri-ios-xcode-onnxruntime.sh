#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="${SCRIPT_DIR}"
while [ "${REPO_ROOT}" != "/" ] && [ ! -f "${REPO_ROOT}/package.json" ]; do
  REPO_ROOT="$(dirname "${REPO_ROOT}")"
done
if [ ! -f "${REPO_ROOT}/package.json" ]; then
  echo "[ios-ort] Failed to locate repository root from ${SCRIPT_DIR}" >&2
  exit 1
fi
export PATH="/opt/homebrew/bin:/usr/local/bin:${HOME}/.bun/bin:${PATH}"

SDKROOT_LOWER="$(printf '%s' "${SDKROOT:-}" | tr '[:upper:]' '[:lower:]')"

if [[ "${SDKROOT_LOWER}" == *"iphonesimulator"* ]]; then
  ORT_SLICE="ios-arm64_x86_64-simulator"
elif [[ "${SDKROOT_LOWER}" == *"iphoneos"* ]]; then
  ORT_SLICE="ios-arm64"
else
  echo "[ios-ort] Unsupported SDKROOT='${SDKROOT:-}'" >&2
  exit 1
fi

if command -v node >/dev/null 2>&1; then
  JS_RUNNER="node"
  NODE_BIN="$(command -v node)"
elif [ -x "/opt/homebrew/bin/node" ]; then
  JS_RUNNER="/opt/homebrew/bin/node"
  NODE_BIN="/opt/homebrew/bin/node"
elif [ -x "/usr/local/bin/node" ]; then
  JS_RUNNER="/usr/local/bin/node"
  NODE_BIN="/usr/local/bin/node"
elif command -v bun >/dev/null 2>&1; then
  JS_RUNNER="bun"
  NODE_BIN=""
elif [ -x "${HOME}/.bun/bin/bun" ]; then
  JS_RUNNER="${HOME}/.bun/bin/bun"
  NODE_BIN=""
else
  echo "[ios-ort] Neither node nor bun is available in PATH." >&2
  exit 1
fi

"${JS_RUNNER}" "${REPO_ROOT}/scripts/install-onnxruntime-ios.mjs" "${ORT_SLICE}"

export ORT_LIB_LOCATION="${REPO_ROOT}/src-tauri/onnxruntime-ios/${ORT_SLICE}"
export ORT_PREFER_DYNAMIC_LINK=0

echo "[ios-ort] ORT_LIB_LOCATION=${ORT_LIB_LOCATION}"

if command -v bun >/dev/null 2>&1; then
  exec bun tauri ios xcode-script "$@"
elif [ -x "${HOME}/.bun/bin/bun" ]; then
  exec "${HOME}/.bun/bin/bun" tauri ios xcode-script "$@"
elif [ -n "${NODE_BIN}" ] && [ -f "${REPO_ROOT}/node_modules/@tauri-apps/cli/tauri.js" ]; then
  exec "${NODE_BIN}" "${REPO_ROOT}/node_modules/@tauri-apps/cli/tauri.js" ios xcode-script "$@"
elif [ -x "${REPO_ROOT}/node_modules/.bin/tauri" ]; then
  exec "${REPO_ROOT}/node_modules/.bin/tauri" ios xcode-script "$@"
elif command -v npx >/dev/null 2>&1; then
  exec npx tauri ios xcode-script "$@"
else
  echo "[ios-ort] Cannot run tauri CLI: bun, local tauri bin, and npx are all unavailable." >&2
  exit 1
fi
