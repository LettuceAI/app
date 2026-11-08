#!/usr/bin/env bash
set -euo pipefail

app=${1:?Usage: bash scripts/macos/verify-signing-identity.sh APP_BUNDLE}
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
expected=$(tr -d '[:space:]' < "$script_dir/signing.sha1")
[[ "$expected" =~ ^[A-F0-9]{40}$ ]] || { echo 'Invalid pinned signing fingerprint.' >&2; exit 1; }
identifier=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist")
[[ "$identifier" == com.lettuceai.app || "$identifier" == dev.lettuceai.app ]] || exit 1
codesign --verify --deep --strict --verbose=2 \
  -R "=identifier \"$identifier\" and certificate leaf = H\"$expected\"" "$app"
requirement=$(codesign -d -r- "$app" 2>&1 | sed -n '/^designated => /p')
normalized=$(printf '%s' "$requirement" | tr '[:lower:]' '[:upper:]')
[[ "$normalized" == *"H\"$expected\""* ]] || {
  echo 'The designated requirement does not pin the signing certificate.' >&2
  exit 1
}
# A build hash in the designated requirement would invalidate Keychain grants after updates.
if [[ "$requirement" == *cdhash* ]]; then
  echo 'The app has a build-specific designated requirement.' >&2
  exit 1
fi
printf '%s\n' "$requirement"
