#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture="$(mktemp -d)"
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/crates/lettuce-app/src" "$fixture/crates/lettuce-model-hub/src" "$fixture/apps/ui/src"
check() {
  bash "$repo_root/scripts/check-hugging-face-paths.sh" "$fixture/crates" "$fixture/apps" >"$fixture/result" 2>&1
}
printf '%s\n' 'const URL: &str = "https://huggingface.co/api/models/kokoro";' > "$fixture/crates/lettuce-model-hub/src/hugging_face.rs"
check
for location in lettuce-app/src/whisper.rs lettuce-app/src/embedding.rs lettuce-model-hub/src/kokoro_install.rs; do
  printf '%s\n' 'const URL: &str = "https://huggingface.co/api/models/catalog";' > "$fixture/crates/$location"
  if check; then
    echo "Hugging Face URL escaped the client boundary: $location" >&2
    exit 1
  fi
  rm -- "$fixture/crates/$location"
done
printf '%s\n' '#[cfg(test)]' 'mod tests {' '    const URL: &str = "https://huggingface.co/api/models/test";' '}' > "$fixture/crates/lettuce-app/src/whisper.rs"
check
printf '%s\n' 'const url = "https://huggingface.co/models";' > "$fixture/apps/ui/src/catalog.ts"
if check; then
  echo "Hugging Face URL escaped into the frontend" >&2
  exit 1
fi
