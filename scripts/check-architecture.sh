#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
crate_root="$repo_root/crates"

mapfile -t manifests < <(find "$crate_root" -mindepth 2 -maxdepth 2 -name Cargo.toml -print | sort)
if [[ ${#manifests[@]} -ne 27 ]]; then
  echo "expected 27 crate manifests, found ${#manifests[@]}" >&2
  exit 1
fi

if [[ -e "$crate_root/lettuce-engine-client" ]]; then
  echo "dead lettuce-engine-client crate must not exist" >&2
  exit 1
fi

if rg -n 'old-code' "$repo_root/Cargo.toml" "$crate_root"/*/Cargo.toml | rg -v '^.*exclude = \["old-code"\]$'; then
  echo "new workspace must not depend on old-code" >&2
  exit 1
fi

check_dependency_owner() {
  local dependency="$1"
  local owner="$2"
  local match

  while IFS= read -r match; do
    [[ -z "$match" ]] && continue
    if [[ "$match" != *"/crates/$owner/Cargo.toml:"* ]]; then
      echo "$dependency is restricted to $owner: $match" >&2
      exit 1
    fi
  done < <(rg -n "^${dependency//-/-}(\.workspace)?[[:space:]]*=" "$crate_root"/*/Cargo.toml || true)
}

check_dependency_owner rusqlite lettuce-database
check_dependency_owner sqlx lettuce-database
check_dependency_owner sea-orm lettuce-database
check_dependency_owner tauri lettuce-app
check_dependency_owner reqwest lettuce-network
check_dependency_owner keyring lettuce-settings
check_dependency_owner cap-std lettuce-platform
check_dependency_owner cap-primitives lettuce-platform

cargo metadata --manifest-path "$repo_root/Cargo.toml" --no-deps --format-version 1 >/dev/null
echo "architecture checks passed"
