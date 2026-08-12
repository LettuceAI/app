#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <summary-file> <artifact>..." >&2
  exit 1
fi

summary_file="$1"
shift

mkdir -p "$(dirname "${summary_file}")"
: > "${summary_file}"

sha256_file() {
  local artifact="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${artifact}" | awk '{print $1}'
    return
  fi

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${artifact}" | awk '{print $1}'
    return
  fi

  echo "no SHA256 tool available (expected sha256sum or shasum)" >&2
  exit 1
}

for artifact in "$@"; do
  if [[ ! -f "${artifact}" ]]; then
    echo "artifact not found: ${artifact}" >&2
    exit 1
  fi

  digest="$(sha256_file "${artifact}")"
  basename="$(basename "${artifact}")"
  printf '%s  %s\n' "${digest}" "${basename}" >> "${summary_file}"
  printf '%s  %s\n' "${digest}" "${basename}" > "${artifact}.sha256"
done
