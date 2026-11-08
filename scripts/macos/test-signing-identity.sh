#!/usr/bin/env bash
set -euo pipefail

: "${APPLE_SIGNING_IDENTITY:?Missing signing identity}"
: "${LETTUCE_SIGNING_KEYCHAIN:?Missing test keychain}"
: "${RUNNER_TEMP:?Missing RUNNER_TEMP}"
staging=$(mktemp -d "$RUNNER_TEMP/lettuce-keychain-smoke.XXXXXX")
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# Rebuild at the same path, just as an app update replaces its executable.
for version in 1 2; do
  clang -DVERSION="$version" -Wno-deprecated-declarations \
    -framework Security -framework CoreFoundation \
    "$script_dir/tests/keychain-roundtrip.c" -o "$staging/keychain-smoke"
  codesign --force --sign "$APPLE_SIGNING_IDENTITY" --keychain "$LETTUCE_SIGNING_KEYCHAIN" \
    --identifier com.lettuceai.signing-smoke --options runtime \
    --entitlements apps/tauri/entitlements.plist "$staging/keychain-smoke"
  codesign --verify --strict "$staging/keychain-smoke"
  codesign -d -r- "$staging/keychain-smoke" > "$staging/requirement-$version" 2>&1
  "$staging/keychain-smoke" "$LETTUCE_SIGNING_KEYCHAIN"
done
cmp "$staging/requirement-1" "$staging/requirement-2"
echo 'Two distinct signed builds shared their Keychain item without user interaction.'
