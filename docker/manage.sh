#!/usr/bin/env bash
set -euo pipefail

directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
action=${1:-status}
service=${2:-vulkan}
case "$service" in cpu|vulkan|cuda|vulkan-nvidia) ;; *) echo "Expected cpu, vulkan, cuda or vulkan-nvidia." >&2; exit 1 ;; esac
case "$action" in status|update|rollback) ;; *) echo "Usage: bash docker/manage.sh <status|update|rollback> <profile>" >&2; exit 1 ;; esac
[[ -f "$directory/.env" ]] || { echo "Copy docker/env.example to docker/.env and set VNC_PW first." >&2; exit 1; }

compose=(docker compose --env-file "$directory/.env" -f "$directory/compose.yaml")
if [[ -n ${LETTUCE_KEYRING_SECRET:-} ]]; then compose+=(-f "$directory/compose.keyring.yaml"); fi
if [[ -n ${LETTUCE_GPU_CONFIG:-} ]]; then compose+=(-f "$LETTUCE_GPU_CONFIG"); fi
compose+=(--profile "$service")
image="lettuceai:kasm-$service"
rollback_image="lettuceai:rollback-$service"

active_image() {
  local container
  container=$("${compose[@]}" ps -q "$service")
  if [[ -n "$container" ]]; then
    docker inspect --format '{{.Image}}' "$container"
  else
    docker image inspect --format '{{.Id}}' "$image"
  fi
}

show_status() {
  "${compose[@]}" ps "$service"
  container=$("${compose[@]}" ps -q "$service")
  if [[ -z "$container" ]]; then echo "The $service container is not running."; return; fi
  docker inspect --format 'Image ID: {{.Image}}' "$container"
  docker exec "$container" /usr/local/bin/lettuceai-container-status
}

case "$action" in
  status) show_status ;;
  update)
    previous=$(active_image 2>/dev/null || true)
    "${compose[@]}" build --pull "$service"
    if [[ -n "$previous" ]]; then docker tag "$previous" "$rollback_image"; fi
    "${compose[@]}" up -d --no-build --force-recreate --wait --wait-timeout 180 "$service"
    show_status
    ;;
  rollback)
    previous=$(docker image inspect --format '{{.Id}}' "$rollback_image")
    current=$(active_image)
    docker tag "$previous" "$image"
    if "${compose[@]}" up -d --no-build --force-recreate --wait --wait-timeout 180 "$service"; then
      docker tag "$current" "$rollback_image"
    else
      docker tag "$current" "$image"
      echo "Rollback could not start. The current image tag has been restored; inspect container logs." >&2
      exit 1
    fi
    show_status
    ;;
esac
