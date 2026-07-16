#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
github_env="${1:-${GITHUB_ENV:-}}"
if [[ -z "$github_env" ]]; then
  printf 'usage: %s GITHUB_ENV_PATH\n' "$0" >&2
  exit 2
fi

postgres_image="postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777"
pgvector_image="pgvector/pgvector@sha256:1d533553fefe4f12e5d80c7b80622ba0c382abb5758856f52983d8789179f0fb"
redis_image="redis@sha256:6ab0b6e7381779332f97b8ca76193e45b0756f38d4c0dcda72dbb3c32061ab99"
nats_image="nats@sha256:c11af972c99ae542de8925e6a7d9c533aa1eb039660420d2074beed6089b3bf0"
openfga_image="openfga/openfga@sha256:8543200bf85878c968d73da46c4f0e31ba1f63ed3675b71122f1133b0e9d97eb"
opa_image="openpolicyagent/opa@sha256:cba27d3c6af2feba1e4d6e6b5e24df5b53db332420d4148a90acccd12efae6ed"

docker run -d --name p02-primary-postgres \
  -e POSTGRES_HOST_AUTH_METHOD=trust \
  -e POSTGRES_DB=p02_identity \
  -p 127.0.0.1:15432:5432 \
  "$pgvector_image"
docker run -d --name p02-witness-postgres \
  -e POSTGRES_HOST_AUTH_METHOD=trust \
  -p 127.0.0.1:15433:5432 \
  "$postgres_image"
docker run -d --name p02-redis \
  -p 127.0.0.1:16379:6379 \
  "$redis_image"
docker run -d --name p02-nats \
  -p 127.0.0.1:14222:4222 \
  -p 127.0.0.1:18222:8222 \
  "$nats_image" -js -m 8222
docker run -d --name p02-openfga \
  -p 127.0.0.1:18080:8080 \
  "$openfga_image" \
  run --datastore-engine memory --playground-enabled=false
docker run -d --name p02-opa \
  -p 127.0.0.1:18082:8181 \
  -v "$root/policy/opa:/policy:ro" \
  "$opa_image" \
  run --server --addr=0.0.0.0:8181 /policy

wait_for_postgres() {
  local container="$1"
  local database="$2"
  local ready=false
  for _ in $(seq 1 120); do
    if docker exec "$container" pg_isready -U postgres -d "$database" >/dev/null 2>&1; then
      ready=true
      break
    fi
    sleep 0.25
  done
  if [[ "$ready" != true ]]; then
    docker logs "$container" >&2
    return 1
  fi
}

wait_for_postgres p02-primary-postgres p02_identity
wait_for_postgres p02-witness-postgres postgres

for database in \
  p02_canonical \
  p02_eventing \
  p02_workflow \
  p02_api_replay \
  p02_formal_commit \
  p03_migration_upgrade; do
  docker exec p02-primary-postgres createdb -U postgres "$database"
done
for database in \
  p02_canonical_witness \
  p02_eventing_witness \
  p02_api_replay_witness \
  p02_formal_commit_witness \
  p02_service_witness; do
  docker exec p02-witness-postgres createdb -U postgres "$database"
done

redis_ready=false
for _ in $(seq 1 120); do
  if [[ "$(docker exec p02-redis redis-cli ping 2>/dev/null || true)" == PONG ]]; then
    redis_ready=true
    break
  fi
  sleep 0.25
done
if [[ "$redis_ready" != true ]]; then
  docker logs p02-redis >&2
  exit 1
fi

nats_ready=false
for _ in $(seq 1 120); do
  if curl -fsS http://127.0.0.1:18222/healthz >/dev/null 2>&1; then
    nats_ready=true
    break
  fi
  sleep 0.25
done
if [[ "$nats_ready" != true ]]; then
  docker logs p02-nats >&2
  exit 1
fi

cat >>"$github_env" <<'ENVIRONMENT'
P02_REDIS_URL=redis://127.0.0.1:16379
P02_CANONICAL_DATABASE_URL=postgresql://postgres@127.0.0.1:15432/p02_canonical
P02_CANONICAL_WITNESS_DATABASE_URL=postgresql://postgres@127.0.0.1:15433/p02_canonical_witness
P02_EVENTING_DATABASE_URL=postgresql://postgres@127.0.0.1:15432/p02_eventing
P02_EVENTING_WITNESS_DATABASE_URL=postgresql://postgres@127.0.0.1:15433/p02_eventing_witness
P02_EVENTING_ALLOW_DATABASE_RESET=1
P02_EVENTING_RESET_DATABASE=p02_eventing
P02_EVENTING_WITNESS_RESET_DATABASE=p02_eventing_witness
P02_WORKFLOW_DATABASE_URL=postgresql://postgres@127.0.0.1:15432/p02_workflow
P02_API_CANONICAL_DATABASE_URL=postgresql://postgres@127.0.0.1:15432/p02_api_replay
P02_API_CANONICAL_WITNESS_DATABASE_URL=postgresql://postgres@127.0.0.1:15433/p02_api_replay_witness
P02_FORMAL_COMMIT_DATABASE_URL=postgresql://postgres@127.0.0.1:15432/p02_formal_commit
P02_FORMAL_COMMIT_WITNESS_DATABASE_URL=postgresql://postgres@127.0.0.1:15433/p02_formal_commit_witness
P02_WITNESS_DATABASE_URL=postgresql://postgres@127.0.0.1:15433/p02_service_witness
P02_NATS_URL=nats://127.0.0.1:14222
P03_DATABASE_URL=postgresql://postgres@127.0.0.1:15432/p03_migration_upgrade
P03_ALLOW_DATABASE_RESET=1
ENVIRONMENT

python3 "$root/scripts/ci/p02_policy_bootstrap.py" --github-env "$github_env"
