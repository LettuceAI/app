#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [[ "${SDKROOT:-}" == *"iphonesimulator"* ]]; then
  ORT_SLICE="ios-arm64_x86_64-simulator"
elif [[ "${SDKROOT:-}" == *"iphoneos"* ]]; then
  ORT_SLICE="ios-arm64"
else
  echo "[ios-ort] Unsupported SDKROOT='${SDKROOT:-}'" >&2
  exit 1
fi

if command -v node >/dev/null 2>&1; then
  JS_RUNNER="node"
elif command -v bun >/dev/null 2>&1; then
  JS_RUNNER="bun"
else
  echo "[ios-ort] Neither node nor bun is available in PATH." >&2
  exit 1
fi

"${JS_RUNNER}" "${REPO_ROOT}/scripts/install-onnxruntime-ios.mjs" "${ORT_SLICE}"

export ORT_LIB_LOCATION="${REPO_ROOT}/src-tauri/onnxruntime-ios/${ORT_SLICE}"
export ORT_PREFER_DYNAMIC_LINK=0

echo "[ios-ort] ORT_LIB_LOCATION=${ORT_LIB_LOCATION}"

exec bun tauri ios xcode-script "$@"
