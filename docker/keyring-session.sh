#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "$1" >&2
  /usr/local/bin/lettuceai-container-status write failed --reason "$1"
  if [[ -n ${DISPLAY:-} ]]; then
    zenity --error --title=LettuceAI --text="$1" || true
  fi
  exit 1
}

provider=${LETTUCE_KEYRING_PROVIDER:-auto}
keyring_timeout=${LETTUCE_KEYRING_TIMEOUT:-120}
[[ "$keyring_timeout" =~ ^[1-9][0-9]*$ ]] || fail "LETTUCE_KEYRING_TIMEOUT must be a positive number of seconds."
/usr/local/bin/lettuceai-container-status write waiting_for_keyring --provider "$provider"
activation_environment=(HOME XDG_DATA_HOME XDG_RUNTIME_DIR)
for variable in DISPLAY WAYLAND_DISPLAY XAUTHORITY XDG_CURRENT_DESKTOP; do
  if [[ -v "$variable" ]]; then activation_environment+=("$variable"); fi
done
dbus-update-activation-environment "${activation_environment[@]}" || fail "Could not configure the desktop D-Bus environment."
case "$provider" in
  gnome)
    command -v gnome-keyring-daemon >/dev/null || fail "GNOME Keyring is not installed in this image."
    install -d -m700 "$XDG_DATA_HOME/keyrings"
    if [[ -n ${LETTUCE_KEYRING_PASSWORD_FILE:-} ]]; then
      [[ -r "$LETTUCE_KEYRING_PASSWORD_FILE" ]] || fail "Cannot read the keyring password file. It must be readable by the Kasm user (UID 1000)."
      keyring_password=$(cat "$LETTUCE_KEYRING_PASSWORD_FILE")
    elif [[ -n ${DISPLAY:-} ]]; then
      keyring_password=$(zenity --password --title="Unlock LettuceAI keyring" --text="Enter your keyring password. On first use, choose a password and use it again next time.") || {
        /usr/local/bin/lettuceai-container-status write stopped --reason 'Unlock cancelled'
        exit 0
      }
    else
      read -rsp 'Keyring password: ' keyring_password
      printf '\n'
    fi
    [[ -n "$keyring_password" ]] || fail "The keyring password must not be empty."
    keyring_control=$(mktemp -d "$XDG_RUNTIME_DIR/lettuceai-keyring.XXXXXX")
    if ! printf '%s' "$keyring_password" | gnome-keyring-daemon --unlock --components=secrets --control-directory="$keyring_control"; then
      unset keyring_password
      fail "GNOME Keyring could not start or unlock."
    fi
    unset keyring_password
    ;;
  auto|external|kwallet)
    [[ -z ${LETTUCE_KEYRING_PASSWORD_FILE:-} ]] || fail "Password-file unlock requires LETTUCE_KEYRING_PROVIDER=gnome; other providers use their native unlock prompt."
    if [[ "$provider" == kwallet ]]; then
      daemon=''
      for candidate in ksecretd kwalletd6 kwalletd5; do
        if command -v "$candidate" >/dev/null; then daemon=$candidate; break; fi
      done
      [[ -n "$daemon" ]] || fail "KWallet Secret Service is not installed in this image."
      "$daemon" &
    fi
    timeout "$keyring_timeout" gdbus wait --session --activate=org.freedesktop.secrets org.freedesktop.secrets \
      || fail "No Secret Service became available on the desktop D-Bus session."
    ;;
  *) fail "Unknown keyring provider: $provider" ;;
esac

/usr/local/bin/lettuceai-container-status write verifying_keyring --provider "$provider"
probe=$(od -An -tx1 -N16 /dev/urandom | tr -d ' \n')
if ! printf '%s' "$probe" | timeout "$keyring_timeout" secret-tool store --label='LettuceAI container check' lettuceai-container readiness instance "$probe"; then
  fail "Keyring is not writable. Check the password, persistent home permissions and container memlock limits."
fi
stored=$(timeout 15s secret-tool lookup lettuceai-container readiness instance "$probe") || fail "Keyring verification failed."
[[ "$stored" == "$probe" ]] || fail "Keyring verification returned an unexpected value."
timeout 15s secret-tool clear lettuceai-container readiness instance "$probe" || fail "Could not finish keyring verification."
unset probe stored
/opt/lettuceai/bin/lettuceai "$@" &
application_pid=$!
/usr/local/bin/lettuceai-container-status write running --pid "$application_pid" --provider "$provider"
trap 'kill -TERM "$application_pid" 2>/dev/null || true' TERM INT
result=0
wait "$application_pid" || result=$?
if kill -0 "$application_pid" 2>/dev/null; then wait "$application_pid" || true; fi
if (( result == 0 )); then
  /usr/local/bin/lettuceai-container-status write stopped --exit-code "$result"
else
  /usr/local/bin/lettuceai-container-status write failed --exit-code "$result"
fi
exit "$result"
