#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 3 || $# -gt 5 ]]; then
  echo "usage: $0 <asset-url> <variant> <output-dir> [pkgrel] [expected-sha256]" >&2
  exit 1
fi

asset_url="$1"
variant="$2"
output_dir="$3"
pkgrel="${4:-1}"
expected_sha256="${5:-${EXPECTED_SHA256:-}}"

case "${variant}" in
  cpu)
    pkgname="lettuceai-bin"
    pkgdesc="LettuceAI desktop app (prebuilt binary, CPU variant)"
    extra_depends=()
    ;;
  vulkan)
    pkgname="lettuceai-vulkan-bin"
    pkgdesc="LettuceAI desktop app (prebuilt binary, Vulkan variant)"
    extra_depends=("vulkan-icd-loader")
    ;;
  cuda)
    pkgname="lettuceai-cuda-bin"
    pkgdesc="LettuceAI desktop app (prebuilt binary, CUDA variant)"
    extra_depends=("cuda")
    ;;
  *)
    echo "unsupported variant: ${variant}" >&2
    exit 1
    ;;
esac

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

archive_path="${tmpdir}/release.tar.gz"
curl -L --fail --proto '=https' --tlsv1.2 -o "${archive_path}" "${asset_url}"
sha256="$(sha256sum "${archive_path}" | awk '{print $1}')"

if [[ -n "${expected_sha256}" ]]; then
  expected_sha256="$(printf '%s' "${expected_sha256}" | tr '[:upper:]' '[:lower:]')"
  if [[ ! "${expected_sha256}" =~ ^[0-9a-f]{64}$ ]]; then
    echo "expected sha256 is not a 64-character hex digest: ${expected_sha256}" >&2
    exit 1
  fi
  if [[ "${sha256}" != "${expected_sha256}" ]]; then
    echo "downloaded asset sha256 mismatch" >&2
    echo "expected: ${expected_sha256}" >&2
    echo "actual:   ${sha256}" >&2
    exit 1
  fi
fi

tar -xzf "${archive_path}" -C "${tmpdir}"
metadata_path="${tmpdir}/.lettuceai-release.json"
if [[ ! -f "${metadata_path}" ]]; then
  echo "release metadata not found in tarball: ${metadata_path}" >&2
  exit 1
fi

pkgver="$(python3 - "${metadata_path}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as fh:
    metadata = json.load(fh)

print(metadata["version"])
PY
)"

mkdir -p "${output_dir}"

depends=(
  "gtk3"
  "webkit2gtk-4.1"
  "libayatana-appindicator"
  "librsvg"
  "speech-dispatcher"
  "openssl"
  "${extra_depends[@]}"
)

pkgbuild_depends=""
srcinfo_depends=""
for dependency in "${depends[@]}"; do
  [[ -z "${dependency}" ]] && continue
  pkgbuild_depends+="  '${dependency}'"$'\n'
  srcinfo_depends+=$'\t'"depends = ${dependency}"$'\n'
done

all_conflicts=("lettuceai-bin" "lettuceai-vulkan-bin" "lettuceai-cuda-bin")
pkgbuild_conflicts=""
srcinfo_conflicts=""
for conflict in "${all_conflicts[@]}"; do
  [[ "${conflict}" == "${pkgname}" ]] && continue
  pkgbuild_conflicts+="  '${conflict}'"$'\n'
  srcinfo_conflicts+=$'\t'"conflicts = ${conflict}"$'\n'
done

source_name="lettuceai-linux-x86_64-${variant}.tar.gz"

cat > "${output_dir}/PKGBUILD" <<EOF
# Maintainer: MegalithOfficial

pkgname=${pkgname}
pkgver=${pkgver}
pkgrel=${pkgrel}
pkgdesc='${pkgdesc}'
arch=('x86_64')
url='https://github.com/LettuceAI/app'
license=('AGPL-3.0-only')
depends=(
${pkgbuild_depends%$'\n'}
)
provides=('lettuceai')
conflicts=(
${pkgbuild_conflicts%$'\n'}
)
source=('${source_name}::${asset_url}')
sha256sums=('${sha256}')

package() {
  install -d "\${pkgdir}/usr"
  cp -a "\${srcdir}/usr/." "\${pkgdir}/usr/"
}
EOF

cat > "${output_dir}/.SRCINFO" <<EOF
pkgbase = ${pkgname}
	pkgdesc = ${pkgdesc}
	pkgver = ${pkgver}
	pkgrel = ${pkgrel}
	url = https://github.com/LettuceAI/app
	arch = x86_64
	license = AGPL-3.0-only
${srcinfo_depends}	provides = lettuceai
${srcinfo_conflicts}	source = ${source_name}::${asset_url}
	sha256sums = ${sha256}

pkgname = ${pkgname}
EOF

cat > "${output_dir}/aur-package.env" <<EOF
PKGNAME=${pkgname}
PKGVER=${pkgver}
PKGREL=${pkgrel}
VARIANT=${variant}
ASSET_URL=${asset_url}
ASSET_SHA256=${sha256}
EOF
