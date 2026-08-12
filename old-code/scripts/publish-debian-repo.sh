#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 4 || $# -gt 7 ]]; then
  echo "usage: $0 <asset-url> <variant> <pages-dir> <base-url> [suite] [component] [origin]" >&2
  exit 1
fi

asset_url="$1"
variant="$2"
pages_dir="$3"
base_url="$4"
suite="${5:-stable}"
component="${6:-main}"
origin="${7:-LettuceAI}"

case "${variant}" in
  cpu|cuda|vulkan) ;;
  *)
    echo "unsupported variant: ${variant}" >&2
    exit 1
    ;;
esac

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

require_cmd curl
require_cmd dpkg-deb
require_cmd dpkg-scanpackages
require_cmd gzip
require_cmd xz
require_cmd sha256sum

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

deb_path="${tmpdir}/package.deb"
curl -L --fail --proto '=https' --tlsv1.2 -o "${deb_path}" "${asset_url}"

pkg_name="$(dpkg-deb -f "${deb_path}" Package)"
pkg_version="$(dpkg-deb -f "${deb_path}" Version)"
pkg_arch="$(dpkg-deb -f "${deb_path}" Architecture)"

repo_root="${pages_dir}/apt/${variant}"
pool_dir="${repo_root}/pool/${component}/${pkg_name}"
dist_dir="${repo_root}/dists/${suite}/${component}/binary-${pkg_arch}"

mkdir -p "${pool_dir}" "${dist_dir}"

asset_name="$(basename "${asset_url%%\?*}")"
if [[ "${asset_name}" != *.deb ]]; then
  asset_name="${pkg_name}_${pkg_version}_${pkg_arch}.deb"
fi

cp "${deb_path}" "${pool_dir}/${asset_name}"

pushd "${repo_root}" >/dev/null
dpkg-scanpackages --multiversion "pool/${component}" > "dists/${suite}/${component}/binary-${pkg_arch}/Packages"
gzip -9c "dists/${suite}/${component}/binary-${pkg_arch}/Packages" > "dists/${suite}/${component}/binary-${pkg_arch}/Packages.gz"
xz -9c "dists/${suite}/${component}/binary-${pkg_arch}/Packages" > "dists/${suite}/${component}/binary-${pkg_arch}/Packages.xz"
popd >/dev/null

release_dir="${repo_root}/dists/${suite}"
release_file="${release_dir}/Release"

architectures="$(find "${release_dir}" -mindepth 2 -maxdepth 2 -type d -name 'binary-*' -printf '%f\n' | sed 's/^binary-//' | sort -u | tr '\n' ' ' | sed 's/ $//')"
components="$(find "${release_dir}" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort -u | tr '\n' ' ' | sed 's/ $//')"
date_rfc2822="$(LC_ALL=C date -Ru)"

cat > "${release_file}" <<EOF
Origin: ${origin}
Label: ${origin} ${variant}
Suite: ${suite}
Codename: ${suite}
Version: 1.0
Architectures: ${architectures}
Components: ${components}
Description: ${origin} Debian repository (${variant} variant)
Date: ${date_rfc2822}
EOF

append_hash_block() {
  local title="$1"
  local hash_cmd="$2"
  {
    echo "${title}:"
    while IFS= read -r relpath; do
      local filepath="${release_dir}/${relpath}"
      local size
      size="$(stat -c '%s' "${filepath}")"
      local hash
      hash="$(${hash_cmd} "${filepath}" | awk '{print $1}')"
      printf " %s %16s %s\n" "${hash}" "${size}" "${relpath}"
    done < <(cd "${release_dir}" && find . -type f ! -name 'Release' ! -name 'InRelease' ! -name 'Release.gpg' | sed 's#^\./##' | sort)
  } >> "${release_file}"
}

append_hash_block "MD5Sum" "md5sum"
append_hash_block "SHA1" "sha1sum"
append_hash_block "SHA256" "sha256sum"

if [[ -n "${DEBIAN_REPO_GPG_PRIVATE_KEY:-}" ]]; then
  require_cmd gpg

  gpg_home="${tmpdir}/gnupg"
  mkdir -p "${gpg_home}"
  chmod 700 "${gpg_home}"
  export GNUPGHOME="${gpg_home}"

  printf '%s' "${DEBIAN_REPO_GPG_PRIVATE_KEY}" | base64 -d | gpg --batch --import
  key_fpr="$(gpg --batch --list-secret-keys --with-colons | awk -F: '/^fpr:/ { print $10; exit }')"
  if [[ -z "${key_fpr}" ]]; then
    echo "failed to load GPG signing key" >&2
    exit 1
  fi

  if [[ -n "${DEBIAN_REPO_GPG_PASSPHRASE:-}" ]]; then
    passphrase_args=(--pinentry-mode loopback --passphrase "${DEBIAN_REPO_GPG_PASSPHRASE}")
  else
    passphrase_args=()
  fi

  gpg --batch --yes "${passphrase_args[@]}" --default-key "${key_fpr}" \
    --clearsign --output "${release_dir}/InRelease" "${release_file}"
  gpg --batch --yes "${passphrase_args[@]}" --default-key "${key_fpr}" \
    --armor --detach-sign --output "${release_dir}/Release.gpg" "${release_file}"
fi

mkdir -p "${repo_root}"
cat > "${repo_root}/index.html" <<EOF
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>${origin} Debian Repo (${variant})</title>
  </head>
  <body>
    <h1>${origin} Debian Repo (${variant})</h1>
    <p>APT source:</p>
    <pre>deb [arch=${pkg_arch}] ${base_url%/}/apt/${variant} ${suite} ${component}</pre>
    <p>Package: ${pkg_name} ${pkg_version}</p>
  </body>
  </html>
EOF

