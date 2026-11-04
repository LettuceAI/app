#!/usr/bin/env bash
set -euo pipefail
touch "$HOME/application-started"
case "${1:-}" in
  write)
    [[ "$2" == 'file with spaces.uec' ]]
    printf '%s' persistent-credential | secret-tool store --label='Container test' lettuceai-test persistent
    ;;
  read)
    [[ $(secret-tool lookup lettuceai-test persistent) == persistent-credential ]]
    ;;
  reuse)
    [[ -n "$DBUS_SESSION_BUS_ADDRESS" ]]
    [[ $(secret-tool lookup lettuceai-test persistent) == persistent-credential ]]
    ;;
  *) echo 'Unexpected application launch.' >&2; exit 1 ;;
esac
