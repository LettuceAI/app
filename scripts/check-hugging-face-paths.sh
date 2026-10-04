#!/usr/bin/env bash
# Usage: check-hugging-face-paths.sh <crates dir> <apps dir>
set -euo pipefail

crate_root="$1"
app_root="$2"

# Hugging Face requests are built and parsed in hugging_face.rs only: no
# other production Rust source may spell its host or an API path in a string
# (format! pieces included), and the frontend never names its host. Only
# `mod` blocks under #[cfg(test)] and test files are skipped.
hf_terms='huggingface\.co|hf\.co|/resolve/|/tree/|/raw/|api/models|/api/whoami|/api/users/|/api/organizations/'
hf_violations=""
while IFS= read -r source; do
  [[ -z "$source" ]] && continue
  case "$source" in
    "$crate_root/lettuce-model-hub/src/hugging_face.rs"|*/tests.rs|*_tests.rs|*/tests/*) continue ;;
  esac
  found="$(HF_TERMS="$hf_terms" awk '
    function strip_raw(text,    start, rest, ending) {
      if (in_raw) {
        ending = index(text, "\"#")
        if (!ending) { return "" }
        text = substr(text, ending + 2)
        in_raw = 0
      }
      while ((start = index(text, "r#\"")) > 0) {
        rest = substr(text, start + 3)
        ending = index(rest, "\"#")
        if (!ending) {
          in_raw = 1
          return substr(text, 1, start - 1)
        }
        text = substr(text, 1, start - 1) substr(rest, ending + 2)
      }
      return text
    }
    function braces(text,    opened, closed) {
      text = strip_raw(text)
      gsub(/\047([^\047\\]|\\.)\047/, "", text)
      gsub(/"([^"\\]|\\.)*"/, "", text)
      sub(/\/\/.*$/, "", text)
      opened = gsub(/\{/, "{", text)
      closed = gsub(/\}/, "}", text)
      return opened - closed
    }
    skipping {
      depth += braces($0)
      if (depth <= 0) { skipping = 0 }
      next
    }
    pending && /^[[:space:]]*$/ { next }
    pending && /^[[:space:]]*#\[/ { next }
    pending && /^[[:space:]]*(pub(\([a-z]+\))?[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*\{/ {
      pending = 0
      depth = braces($0)
      if (depth > 0) { skipping = 1 }
      next
    }
    { pending = 0 }
    /^[[:space:]]*#\[cfg\(test\)\]/ { pending = 1; next }
    /^[[:space:]]*\/\// { next }
    {
      line = $0
      while (match(line, /"([^"\\]|\\.)*"/)) {
        literal = substr(line, RSTART, RLENGTH)
        if (literal ~ ENVIRON["HF_TERMS"]) { print FILENAME ":" FNR ": " $0; break }
        line = substr(line, RSTART + RLENGTH)
      }
    }
  ' "$source")"
  if [[ -n "$found" ]]; then
    hf_violations+="$found"$'\n'
  fi
done < <(rg -l -e "$hf_terms" --glob '*.rs' "$crate_root" "$app_root" || true)
ui_source="$app_root/ui/src"
if [[ -d "$ui_source" ]]; then
  ui_found="$(rg -n -e 'huggingface\.co|hf\.co' "$ui_source" --glob '!**/api/generated/**' || true)"
  if [[ -n "$ui_found" ]]; then
    hf_violations+="$ui_found"$'\n'
  fi
fi
if [[ -n "$hf_violations" ]]; then
  printf '%s' "$hf_violations" >&2
  echo "Hugging Face hosts and API paths belong to crates/lettuce-model-hub/src/hugging_face.rs" >&2
  exit 1
fi
