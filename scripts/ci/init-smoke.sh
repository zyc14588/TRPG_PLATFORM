#!/usr/bin/env sh
set -eu

mkdir -p evidence/stages/S09
curl -fsS http://localhost:8080/healthz >/dev/null
printf 'healthz http://localhost:8080/healthz => 200\n' > evidence/stages/S09/docker-compose-smoke.txt
