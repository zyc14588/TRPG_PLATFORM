#!/usr/bin/env bash
set -euo pipefail

for container in \
  trpg-p07-primary-postgres \
  trpg-p07-witness-postgres \
  trpg-p07-openfga \
  trpg-p07-opa \
  trpg-p07-nats; do
  if docker container inspect "$container" >/dev/null 2>&1; then
    docker rm -f "$container" >/dev/null
  fi
done
