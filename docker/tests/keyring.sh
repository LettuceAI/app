#!/usr/bin/env bash
set -euo pipefail
umask 077
password_file=$(mktemp)
printf '%s' container-test-passphrase > "$password_file"
export LETTUCE_KEYRING_PASSWORD_FILE="$password_file"
export LETTUCE_KEYRING_PROVIDER=gnome LETTUCE_KEYRING_TIMEOUT=5

lettuceai write 'file with spaces.uec'
lettuceai read
if grep -r -q persistent-credential "$HOME/.local/share/keyrings"; then
  echo 'Credential was written in plaintext.' >&2
  exit 1
fi
rm "$HOME/application-started"
printf '%s' incorrect-passphrase > "$password_file"
if lettuceai must-not-start; then echo 'Wrong password was accepted.' >&2; exit 1; fi
[[ ! -e "$HOME/application-started" ]]
lettuceai-container-status | python3 -c 'import json,sys; assert json.load(sys.stdin)["phase"] == "failed"'

printf '%s' container-test-passphrase > "$password_file"
lettuceai read
export XDG_DATA_HOME="$HOME/.local/share"
export XDG_RUNTIME_DIR
XDG_RUNTIME_DIR=$(mktemp -d)
export TEST_PASSWORD_FILE="$password_file"
reuse_service() {
  cat "$TEST_PASSWORD_FILE" | gnome-keyring-daemon --unlock --components=secrets
  export LETTUCE_KEYRING_PROVIDER=external
  unset LETTUCE_KEYRING_PASSWORD_FILE
  lettuceai reuse
}
export -f reuse_service
dbus-run-session -- bash -ec reuse_service
lettuceai-container-status | python3 -c 'import json,sys; assert json.load(sys.stdin)["phase"] == "stopped"'
echo 'PASS: encrypted persistence, wrong-password rejection, argument forwarding, status and existing Secret Service reuse.'
