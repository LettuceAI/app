#!/usr/bin/env bash
set -euo pipefail

image=${1:-lettuceai-keyring-test}
volume=$(docker volume create --label lettuceai.test=keyring-persistence)
trap 'docker volume rm "$volume" >/dev/null' EXIT
for mode in write read; do
  docker run --rm -i --cap-add IPC_LOCK --ulimit memlock=-1:-1 \
    --mount "type=volume,source=$volume,target=/home/lettuce-test" \
    -e TEST_MODE="$mode" "$image" bash -se <<'SESSION'
umask 077
if [[ "$TEST_MODE" == read ]]; then
  lettuceai-container-status | python3 -c 'import json,sys; assert json.load(sys.stdin)["phase"] == "not_started"'
fi
password_file=$(mktemp)
printf '%s' persistent-container-passphrase > "$password_file"
export LETTUCE_KEYRING_PASSWORD_FILE="$password_file"
export LETTUCE_KEYRING_PROVIDER=gnome LETTUCE_KEYRING_TIMEOUT=5
lettuceai "$TEST_MODE" 'file with spaces.uec'
SESSION
done
echo 'PASS: encrypted credentials survive container replacement; old status is not reported as current.'
