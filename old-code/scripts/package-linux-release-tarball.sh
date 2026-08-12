#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 <bundle-dir> <variant> <output-tarball>" >&2
  exit 1
fi

bundle_dir="$1"
variant="$2"
output_tarball="$3"

deb_path="$(find "${bundle_dir}/deb" -maxdepth 1 -type f -name '*.deb' | sort | head -n 1)"
if [[ -z "${deb_path}" ]]; then
  echo "no .deb found under ${bundle_dir}/deb" >&2
  exit 1
fi

package_name="$(dpkg-deb -f "${deb_path}" Package)"
package_version="$(dpkg-deb -f "${deb_path}" Version)"
architecture="$(dpkg-deb -f "${deb_path}" Architecture)"

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

payload_dir="${tmpdir}/payload"
mkdir -p "${payload_dir}"
dpkg-deb -x "${deb_path}" "${payload_dir}"

cat > "${payload_dir}/.lettuceai-release.json" <<EOF
{
  "package": "${package_name}",
  "version": "${package_version}",
  "architecture": "${architecture}",
  "variant": "${variant}",
  "source_deb": "$(basename "${deb_path}")"
}
EOF

mkdir -p "$(dirname "${output_tarball}")"
tar -C "${payload_dir}" -czf "${output_tarball}" .
