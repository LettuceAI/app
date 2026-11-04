#!/usr/bin/env bash
set -euo pipefail

provider=${1:?Expected gnome, kwallet or external}
export DEBIAN_FRONTEND=noninteractive
case "$provider" in
  gnome)
    apt-get update
    apt-get install -y --no-install-recommends gnome-keyring
    ;;
  kwallet)
    apt-get update
    apt-get install -y --no-install-recommends libkf5wallet-bin kwalletmanager
    install -d /usr/share/dbus-1/services
    printf '%s\n' '[D-BUS Service]' 'Name=org.freedesktop.secrets' \
      'Exec=/usr/bin/kwalletd5' > /usr/share/dbus-1/services/org.freedesktop.secrets.service
    ;;
  external) ;;
  *) echo "Unsupported keyring provider: $provider" >&2; exit 1 ;;
esac
install -d /opt/lettuceai
printf '%s\n' "$provider" > /opt/lettuceai/keyring-provider
rm -rf /var/lib/apt/lists/*
