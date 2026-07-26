#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
environment_file="${1:-}"
if [[ -z "$environment_file" ]]; then
  printf 'usage: %s ENVIRONMENT_FILE\n' "$0" >&2
  exit 2
fi

primary_name="trpg-p07-primary-postgres"
witness_name="trpg-p07-witness-postgres"
openfga_name="trpg-p07-openfga"
opa_name="trpg-p07-opa"
nats_name="trpg-p07-nats"
for container in "$primary_name" "$witness_name" "$openfga_name" "$opa_name" "$nats_name"; do
  if docker container inspect "$container" >/dev/null 2>&1; then
    printf 'P07 integration container already exists: %s\n' "$container" >&2
    exit 1
  fi
done

postgres_image="${TRPG_INTEGRATION_POSTGRES_IMAGE:-postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777}"
pgvector_image="${TRPG_INTEGRATION_PGVECTOR_IMAGE:-pgvector/pgvector@sha256:12a379b47ad65289572ea0756efc11b7c241a6662833e8af7038cd3b73d647e0}"
openfga_image="${TRPG_INTEGRATION_OPENFGA_IMAGE:-openfga/openfga@sha256:8543200bf85878c968d73da46c4f0e31ba1f63ed3675b71122f1133b0e9d97eb}"
opa_image="${TRPG_INTEGRATION_OPA_IMAGE:-openpolicyagent/opa@sha256:cba27d3c6af2feba1e4d6e6b5e24df5b53db332420d4148a90acccd12efae6ed}"
nats_image="${TRPG_INTEGRATION_NATS_IMAGE:-nats@sha256:c11af972c99ae542de8925e6a7d9c533aa1eb039660420d2074beed6089b3bf0}"
postgres_password="$(openssl rand -hex 24)"

cleanup_on_failure() {
  local status=$?
  if [[ "$status" -ne 0 ]]; then
    docker rm -f \
      "$primary_name" "$witness_name" "$openfga_name" "$opa_name" "$nats_name" \
      >/dev/null 2>&1 || true
  fi
  exit "$status"
}
trap cleanup_on_failure EXIT

docker run -d --name "$primary_name" \
  -e "POSTGRES_PASSWORD=$postgres_password" \
  -e "POSTGRES_INITDB_ARGS=--auth-host=scram-sha-256" \
  -e POSTGRES_DB=p07_player_action \
  -p 127.0.0.1:15442:5432 \
  "$pgvector_image" >/dev/null
docker run -d --name "$witness_name" \
  -e "POSTGRES_PASSWORD=$postgres_password" \
  -e "POSTGRES_INITDB_ARGS=--auth-host=scram-sha-256" \
  -e POSTGRES_DB=p07_player_action_witness \
  -p 127.0.0.1:15443:5432 \
  "$postgres_image" >/dev/null
docker run -d --name "$openfga_name" \
  -p 127.0.0.1:18090:8080 \
  "$openfga_image" \
  run --datastore-engine memory --playground-enabled=false >/dev/null
docker run -d --name "$opa_name" \
  -p 127.0.0.1:18092:8181 \
  -v "$root/policy/opa:/policy:ro" \
  "$opa_image" \
  run --server --addr=0.0.0.0:8181 /policy >/dev/null
docker run -d --name "$nats_name" \
  -p 127.0.0.1:14227:4222 \
  -p 127.0.0.1:18227:8222 \
  "$nats_image" -js -m 8222 >/dev/null

wait_for_postgres() {
  local container="$1"
  local database="$2"
  for _ in $(seq 1 120); do
    if docker exec "$container" \
      psql -X -v ON_ERROR_STOP=1 -U postgres -d "$database" \
      -c 'SELECT 1' >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  docker logs "$container" >&2
  return 1
}
wait_for_postgres "$primary_name" p07_player_action
wait_for_postgres "$witness_name" p07_player_action_witness

for _ in $(seq 1 120); do
  if curl --fail --silent --show-error http://127.0.0.1:18227/healthz >/dev/null 2>&1; then
    nats_ready=true
    break
  fi
  sleep 0.25
done
if [[ "${nats_ready:-false}" != true ]]; then
  docker logs "$nats_name" >&2
  exit 1
fi

docker exec -i "$primary_name" \
  psql -X -v ON_ERROR_STOP=1 -U postgres -d p07_player_action \
  <"$root/scripts/ci/bootstrap-integration-database-roles.sql" >/dev/null

umask 077
: >"$environment_file"
{
  printf 'P07_DATABASE_URL=postgresql://postgres:%s@127.0.0.1:15442/p07_player_action\n' \
    "$postgres_password"
  printf 'P07_WITNESS_DATABASE_URL=postgresql://postgres:%s@127.0.0.1:15443/p07_player_action_witness\n' \
    "$postgres_password"
  printf 'P07_ALLOW_DATABASE_RESET=1\n'
  printf 'P07_RESET_DATABASE=p07_player_action\n'
  printf 'P07_WITNESS_RESET_DATABASE=p07_player_action_witness\n'
  printf 'P07_NATS_URL=nats://127.0.0.1:14227\n'
} >>"$environment_file"

python3 "$root/scripts/ci/p02_policy_bootstrap.py" \
  --openfga-address 127.0.0.1:18090 \
  --opa-address 127.0.0.1:18092 \
  --store-name p07-player-action \
  --github-env "$environment_file" >/dev/null

chmod 0600 "$environment_file"
trap - EXIT
printf 'P07 integration services are ready; environment written with mode 0600.\n'
