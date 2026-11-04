#!/usr/bin/env bash
set -euo pipefail
if [[ ${LETTUCE_AUTOSTART:-1} == 1 ]]; then
  exec /usr/local/bin/lettuceai
fi
