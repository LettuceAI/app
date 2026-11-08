#!/usr/bin/env bash
set -euo pipefail
umask 077

: "${MACOS_SIGNING_CERTIFICATE:?Missing MACOS_SIGNING_CERTIFICATE (base64 PKCS#12)}"
: "${MACOS_SIGNING_PASSWORD:?Missing MACOS_SIGNING_PASSWORD}"
: "${RUNNER_TEMP:?Missing RUNNER_TEMP}"
: "${GITHUB_ENV:?Missing GITHUB_ENV}"
[[ $(uname -s) == Darwin ]] || { echo 'Import requires macOS.' >&2; exit 1; }

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
expected=$(tr -d '[:space:]' < "$script_dir/signing.sha1")
[[ "$expected" =~ ^[A-F0-9]{40}$ ]] || { echo 'Invalid pinned signing fingerprint.' >&2; exit 1; }
staging=$(mktemp -d "$RUNNER_TEMP/lettuce-macos-signing.XXXXXX")
certificate="$staging/certificate.pem"
keychain="$staging/signing.keychain-db"
trap 'rm -f "$staging/certificate.p12"' EXIT
# Decode without printing the certificate or its private key.
printf '%s' "$MACOS_SIGNING_CERTIFICATE" | openssl base64 -d -A > "$staging/certificate.p12"
export MACOS_SIGNING_PASSWORD
openssl pkcs12 -in "$staging/certificate.p12" -passin env:MACOS_SIGNING_PASSWORD \
  -clcerts -nokeys -out "$certificate"
actual=$(openssl x509 -in "$certificate" -noout -fingerprint -sha1 | cut -d= -f2 | tr -d ':')
[[ "$actual" == "$expected" ]] || { echo 'Signing certificate differs from the pinned identity.' >&2; exit 1; }
# Register cleanup before changing the runner's keychain or trust settings.
{
  printf 'LETTUCE_SIGNING_KEYCHAIN=%s\n' "$keychain"
  printf 'LETTUCE_SIGNING_CERTIFICATE_PATH=%s\n' "$certificate"
} >> "$GITHUB_ENV"
keychain_password=$(openssl rand -base64 48)
printf '::add-mask::%s\n' "$keychain_password"
security create-keychain -p "$keychain_password" "$keychain"
security unlock-keychain -p "$keychain_password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security import "$staging/certificate.p12" -k "$keychain" \
  -P "$MACOS_SIGNING_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple:,codesign: -s \
  -k "$keychain_password" "$keychain" >/dev/null
# Trust for signing only, on the disposable CI runner. Users install no root certificate.
sudo security add-trusted-cert -d -r trustRoot -p codeSign -k "$keychain" "$certificate"
python3 - "$keychain" <<'PY'
import shlex
import subprocess
import sys
previous = shlex.split(subprocess.check_output(['security', 'list-keychains', '-d', 'user'], text=True))
subprocess.run(['security', 'list-keychains', '-d', 'user', '-s', sys.argv[1], *previous], check=True)
PY
# Use a pre-imported identity: Tauri's APPLE_CERTIFICATE importer only finds Apple-named certs.
printf 'APPLE_SIGNING_IDENTITY=%s\n' "$expected" >> "$GITHUB_ENV"
printf 'Configured the pinned LettuceAI signing identity: %s\n' "$expected"
