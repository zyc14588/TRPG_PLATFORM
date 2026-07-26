#!/usr/bin/env bash
set -euo pipefail

tool_name="$(basename -- "$0")"
if [[ "$tool_name" != "pg_dump" && "$tool_name" != "pg_restore" ]]; then
  printf 'unsupported PostgreSQL client wrapper name: %s\n' "$tool_name" >&2
  exit 2
fi

client_image="${TRPG_POSTGRES_CLIENT_IMAGE:-}"
if [[ ! "$client_image" =~ ^[a-z0-9][a-z0-9._:/-]*@sha256:[0-9a-f]{64}$ ]]; then
  printf 'TRPG_POSTGRES_CLIENT_IMAGE must be pinned by sha256 digest\n' >&2
  exit 2
fi

mount_root="${TRPG_POSTGRES_CLIENT_MOUNT_ROOT:-}"
if [[ "$mount_root" != /* || "$mount_root" == "/" || ! -d "$mount_root" ||
      -L "$mount_root" || "$mount_root" == *","* ||
      "$mount_root" == *$'\n'* || "$mount_root" == *$'\r'* ]]; then
  printf 'TRPG_POSTGRES_CLIENT_MOUNT_ROOT must be an absolute non-symlink directory\n' >&2
  exit 2
fi
mount_root="${mount_root%/}"
canonical_mount_root="$(realpath -e -- "$mount_root")"
if [[ "$canonical_mount_root" != "$mount_root" ]]; then
  printf 'TRPG_POSTGRES_CLIENT_MOUNT_ROOT must use its canonical path\n' >&2
  exit 2
fi

path_is_mounted() {
  local candidate="$1"
  [[ "$candidate" != *"/../"* && "$candidate" != */.. &&
     ("$candidate" == "$mount_root" || "$candidate" == "$mount_root/"*) ]]
}

for environment_name in PGSERVICEFILE PGPASSFILE; do
  if [[ -v "$environment_name" ]]; then
    environment_path="${!environment_name}"
    if [[ -n "$environment_path" ]] && ! path_is_mounted "$environment_path"; then
      printf '%s must remain inside the PostgreSQL client mount root\n' \
        "$environment_name" >&2
      exit 2
    fi
  fi
done

for argument in "$@"; do
  argument_path=""
  if [[ "$argument" == /* ]]; then
    argument_path="$argument"
  elif [[ "$argument" == *=/* ]]; then
    argument_path="${argument#*=}"
  elif [[ "$argument" == -f/* ]]; then
    argument_path="${argument#-f}"
  fi
  if [[ -n "$argument_path" ]] && ! path_is_mounted "$argument_path"; then
    printf 'PostgreSQL client file argument escapes the mount root\n' >&2
    exit 2
  fi
done

docker_arguments=(
  run
  --rm
  --init
  --network host
  --read-only
  --cap-drop ALL
  --security-opt no-new-privileges
  --pids-limit 64
  --user "$(id -u):$(id -g)"
  --env "HOME=$mount_root"
  --mount "type=bind,src=$mount_root,dst=$mount_root"
)

for environment_name in PGSERVICEFILE PGSERVICE PGCONNECT_TIMEOUT PGPASSFILE PGSSLMODE; do
  if [[ -v "$environment_name" && -n "${!environment_name}" ]]; then
    docker_arguments+=(--env "$environment_name")
  fi
done

exec docker "${docker_arguments[@]}" "$client_image" "$tool_name" "$@"
