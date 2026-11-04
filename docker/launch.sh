#!/usr/bin/env bash
set -euo pipefail

if [[ $(id -u) == 0 ]]; then
  echo "Launch LettuceAI as the Kasm desktop user, not root." >&2
  exit 1
fi
umask 077
export XDG_DATA_HOME=${XDG_DATA_HOME:-$HOME/.local/share}
install -d -m700 "$XDG_DATA_HOME"
/usr/local/bin/lettuceai-container-status write starting
if [[ -z ${XDG_RUNTIME_DIR:-} || ! -d "$XDG_RUNTIME_DIR" || ! -w "$XDG_RUNTIME_DIR" ]]; then
  XDG_RUNTIME_DIR=$(mktemp -d /tmp/lettuceai-runtime.XXXXXX)
  export XDG_RUNTIME_DIR
  trap 'rmdir "$XDG_RUNTIME_DIR" 2>/dev/null || true' EXIT
fi
provider=${LETTUCE_KEYRING_PROVIDER:-auto}
if [[ "$provider" != gnome && -n ${DBUS_SESSION_BUS_ADDRESS:-} ]] && \
   timeout 5s gdbus call --session --dest org.freedesktop.DBus \
     --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.ListNames >/dev/null 2>&1; then
  exec /usr/local/lib/lettuceai/keyring-session "$@"
fi
unset GNOME_KEYRING_CONTROL GNOME_KEYRING_PID DBUS_STARTER_ADDRESS DBUS_STARTER_BUS_TYPE
dbus-run-session -- /usr/local/lib/lettuceai/keyring-session "$@"
