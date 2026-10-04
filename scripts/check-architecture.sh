#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
crate_root="$repo_root/crates"
app_root="$repo_root/apps"

mapfile -t manifests < <(find "$crate_root" -mindepth 2 -maxdepth 2 -name Cargo.toml -print | sort)
if [[ ${#manifests[@]} -ne 27 ]]; then
  echo "expected 27 crate manifests, found ${#manifests[@]}" >&2
  exit 1
fi

mapfile -t app_manifests < <(find "$app_root" -mindepth 2 -maxdepth 2 -name Cargo.toml -print | sort)
if [[ "${app_manifests[*]}" != "$app_root/tauri/Cargo.toml" ]]; then
  echo "expected only the apps/tauri manifest, found: ${app_manifests[*]}" >&2
  exit 1
fi
all_manifests=("${manifests[@]}" "${app_manifests[@]}")

if [[ -e "$crate_root/lettuce-engine-client" ]]; then
  echo "dead lettuce-engine-client crate must not exist" >&2
  exit 1
fi

if rg -n 'old-code' "$repo_root/Cargo.toml" "${all_manifests[@]}" | rg -v '^.*exclude = \["old-code"\]$'; then
  echo "new workspace must not depend on old-code" >&2
  exit 1
fi

if rg -n '^lettuce-tauri(\.workspace)?[[:space:]]*=' "${all_manifests[@]}"; then
  echo "nothing may depend on the Tauri shell" >&2
  exit 1
fi

check_dependency_owner() {
  local dependency="$1"
  shift
  local match owner allowed

  while IFS= read -r match; do
    [[ -z "$match" ]] && continue
    allowed=false
    for owner in "$@"; do
      if [[ "$match" == "$repo_root/$owner/Cargo.toml:"* ]]; then
        allowed=true
      fi
    done
    if [[ "$allowed" == false ]]; then
      echo "$dependency is restricted to $*: $match" >&2
      exit 1
    fi
  done < <(rg -n "^${dependency}(\.workspace)?[[:space:]]*=" "${all_manifests[@]}" || true)
}

check_dependency_owner rusqlite crates/lettuce-database
check_dependency_owner sqlx crates/lettuce-database
check_dependency_owner sea-orm crates/lettuce-database
check_dependency_owner tauri apps/tauri
check_dependency_owner tauri-build apps/tauri
check_dependency_owner tauri-specta apps/tauri
check_dependency_owner specta crates/lettuce-contracts apps/tauri
check_dependency_owner specta-typescript crates/lettuce-contracts apps/tauri
check_dependency_owner reqwest crates/lettuce-network
check_dependency_owner keyring crates/lettuce-settings
check_dependency_owner cap-std crates/lettuce-platform
check_dependency_owner cap-primitives crates/lettuce-platform

bash "$repo_root/scripts/check-hugging-face-paths.sh" "$crate_root" "$app_root"
bash "$repo_root/scripts/tests/check-hugging-face-paths.sh"

app_tree="$(cargo tree --manifest-path "$repo_root/Cargo.toml" -p lettuce-app --edges normal,build --prefix none --offline)"
if rg -q '^(tauri|tauri-[a-z-]+|wry|tao|specta|specta-[a-z-]+) v' <<<"$app_tree"; then
  echo "lettuce-app must stay free of Tauri and specta" >&2
  exit 1
fi

cargo metadata --manifest-path "$repo_root/Cargo.toml" --no-deps --format-version 1 >/dev/null
echo "architecture checks passed"
