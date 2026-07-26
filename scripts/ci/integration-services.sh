#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
github_env="${1:-${GITHUB_ENV:-}}"
if [[ -z "$github_env" ]]; then
  printf 'usage: %s GITHUB_ENV_PATH\n' "$0" >&2
  exit 2
fi
touch "$github_env"
chmod 0600 "$github_env"

postgres_image="${TRPG_INTEGRATION_POSTGRES_IMAGE:-postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777}"
pgvector_image="${TRPG_INTEGRATION_PGVECTOR_IMAGE:-pgvector/pgvector@sha256:12a379b47ad65289572ea0756efc11b7c241a6662833e8af7038cd3b73d647e0}"
redis_image="redis@sha256:6ab0b6e7381779332f97b8ca76193e45b0756f38d4c0dcda72dbb3c32061ab99"
nats_image="nats@sha256:c11af972c99ae542de8925e6a7d9c533aa1eb039660420d2074beed6089b3bf0"
openfga_image="openfga/openfga@sha256:8543200bf85878c968d73da46c4f0e31ba1f63ed3675b71122f1133b0e9d97eb"
opa_image="openpolicyagent/opa@sha256:cba27d3c6af2feba1e4d6e6b5e24df5b53db332420d4148a90acccd12efae6ed"
minio_image="minio/minio@sha256:a1ea29fa28355559ef137d71fc570e508a214ec84ff8083e39bc5428980b015e"
minio_client_image="minio/mc@sha256:aead63c77f9db9107f1696fb08ecb0faeda23729cde94b0f663edf4fe09728e3"
runtime_root="${RUNNER_TEMP:-}"
if [[ -z "$runtime_root" ]]; then
  runtime_root="$(mktemp -d)"
fi
tls_directory="$(mktemp -d "$runtime_root/trpg-postgres-tls.XXXXXX")"
backup_directory="$(mktemp -d "$runtime_root/trpg-backup.XXXXXX")"
libpq_service_file="$backup_directory/pg_service.conf"
minio_access_key="trpg_ci_access"
minio_secret_key="trpg_ci_secret_key_20260725"
minio_bucket="trpg-ci-deletion"
tls_hostname="localhost"
postgres_password="$(openssl rand -hex 24)"

openssl req -x509 -newkey rsa:2048 -nodes -sha256 -days 1 \
  -subj "/CN=TRPG CI PostgreSQL Root" \
  -keyout "$tls_directory/ca.key" \
  -out "$tls_directory/ca.crt" >/dev/null 2>&1
openssl req -newkey rsa:2048 -nodes -sha256 \
  -subj "/CN=$tls_hostname" \
  -keyout "$tls_directory/server.key" \
  -out "$tls_directory/server.csr" >/dev/null 2>&1
printf 'subjectAltName=DNS:%s\nextendedKeyUsage=serverAuth\n' "$tls_hostname" \
  >"$tls_directory/server.ext"
openssl x509 -req -sha256 -days 1 \
  -in "$tls_directory/server.csr" \
  -CA "$tls_directory/ca.crt" \
  -CAkey "$tls_directory/ca.key" \
  -CAcreateserial \
  -extfile "$tls_directory/server.ext" \
  -out "$tls_directory/server.crt" >/dev/null 2>&1
chmod 0600 "$tls_directory/ca.key" "$tls_directory/server.key"
chmod 0644 "$tls_directory/ca.crt" "$tls_directory/server.crt"
{
  printf 'local all all trust\n'
  printf 'hostnossl all all 0.0.0.0/0 reject\n'
  printf 'hostnossl all all ::0/0 reject\n'
  printf 'hostssl all all 0.0.0.0/0 scram-sha-256\n'
  printf 'hostssl all all ::0/0 scram-sha-256\n'
} >"$tls_directory/pg_hba.conf"
chmod 0644 "$tls_directory/pg_hba.conf"

docker run -d --name trpg-primary-postgres \
  -e "POSTGRES_PASSWORD=$postgres_password" \
  -e "POSTGRES_INITDB_ARGS=--auth-host=scram-sha-256" \
  -e POSTGRES_DB=p02_identity \
  -p 127.0.0.1:15432:5432 \
  "$pgvector_image"
docker run -d --name trpg-witness-postgres \
  -e "POSTGRES_PASSWORD=$postgres_password" \
  -e "POSTGRES_INITDB_ARGS=--auth-host=scram-sha-256" \
  -p 127.0.0.1:15433:5432 \
  "$postgres_image"
docker run -d --name trpg-tls-postgres \
  -e "POSTGRES_PASSWORD=$postgres_password" \
  -e "POSTGRES_INITDB_ARGS=--auth-host=scram-sha-256" \
  -e POSTGRES_DB=p02_tls_identity \
  -p 127.0.0.1:15434:5432 \
  "$pgvector_image"
docker run -d --name trpg-redis \
  -p 127.0.0.1:16379:6379 \
  "$redis_image"
docker run -d --name trpg-nats \
  -p 127.0.0.1:14222:4222 \
  -p 127.0.0.1:18222:8222 \
  "$nats_image" -js -m 8222
docker run -d --name trpg-openfga \
  -p 127.0.0.1:18080:8080 \
  "$openfga_image" \
  run --datastore-engine memory --playground-enabled=false
docker run -d --name trpg-opa \
  -p 127.0.0.1:18082:8181 \
  -v "$root/policy/opa:/policy:ro" \
  "$opa_image" \
  run --server --addr=0.0.0.0:8181 /policy
docker run -d --name trpg-minio \
  -e "MINIO_ROOT_USER=$minio_access_key" \
  -e "MINIO_ROOT_PASSWORD=$minio_secret_key" \
  -p 127.0.0.1:19000:9000 \
  "$minio_image" \
  server /data --console-address :9001

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

wait_for_postgres trpg-primary-postgres p02_identity
wait_for_postgres trpg-witness-postgres postgres
wait_for_postgres trpg-tls-postgres p02_tls_identity

for database in \
  p02_canonical \
  p02_eventing \
  p02_workflow \
  p02_api_replay \
  p02_formal_commit \
  p03_migration_upgrade \
  p04_eventing \
  p04_eventing_recovery \
  p05_privacy \
  p06_core_domain \
  p07_player_action \
  trpg_backup_source \
  trpg_backup_target; do
  docker exec trpg-primary-postgres createdb -U postgres "$database"
done
for database in \
  p02_canonical_witness \
  p02_eventing_witness \
  p02_api_replay_witness \
  p02_formal_commit_witness \
  p02_service_witness \
  p04_eventing_witness \
  p05_privacy_witness \
  p06_core_domain_witness \
  p07_player_action_witness; do
  docker exec trpg-witness-postgres createdb -U postgres "$database"
done

docker exec -i trpg-primary-postgres \
  psql -X -v ON_ERROR_STOP=1 -U postgres -d p03_migration_upgrade \
  <"$root/scripts/ci/bootstrap-integration-database-roles.sql"

docker exec trpg-tls-postgres install -d -m 0700 -o postgres -g postgres /var/lib/postgresql/tls
docker cp "$tls_directory/server.crt" trpg-tls-postgres:/var/lib/postgresql/tls/server.crt
docker cp "$tls_directory/server.key" trpg-tls-postgres:/var/lib/postgresql/tls/server.key
docker cp "$tls_directory/pg_hba.conf" trpg-tls-postgres:/var/lib/postgresql/tls/pg_hba.conf
docker exec trpg-tls-postgres chown postgres:postgres \
  /var/lib/postgresql/tls/pg_hba.conf \
  /var/lib/postgresql/tls/server.crt \
  /var/lib/postgresql/tls/server.key
docker exec trpg-tls-postgres chmod 0644 /var/lib/postgresql/tls/pg_hba.conf
docker exec trpg-tls-postgres chmod 0644 /var/lib/postgresql/tls/server.crt
docker exec trpg-tls-postgres chmod 0600 /var/lib/postgresql/tls/server.key
docker exec trpg-tls-postgres psql -v ON_ERROR_STOP=1 -U postgres -d p02_tls_identity \
  -c "ALTER SYSTEM SET ssl = 'on'" \
  -c "ALTER SYSTEM SET ssl_cert_file = '/var/lib/postgresql/tls/server.crt'" \
  -c "ALTER SYSTEM SET ssl_key_file = '/var/lib/postgresql/tls/server.key'" \
  -c "ALTER SYSTEM SET ssl_min_protocol_version = 'TLSv1.2'" \
  -c "ALTER SYSTEM SET hba_file = '/var/lib/postgresql/tls/pg_hba.conf'" >/dev/null
docker restart trpg-tls-postgres >/dev/null
wait_for_postgres trpg-tls-postgres p02_tls_identity
redis_ready=false
for _ in $(seq 1 120); do
  if [[ "$(docker exec trpg-redis redis-cli ping 2>/dev/null || true)" == PONG ]]; then
    redis_ready=true
    break
  fi
  sleep 0.25
done
if [[ "$redis_ready" != true ]]; then
  docker logs trpg-redis >&2
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
  docker logs trpg-nats >&2
  exit 1
fi

minio_ready=false
for _ in $(seq 1 120); do
  if curl -fsS http://127.0.0.1:19000/minio/health/live >/dev/null 2>&1; then
    minio_ready=true
    break
  fi
  sleep 0.25
done
if [[ "$minio_ready" != true ]]; then
  docker logs trpg-minio >&2
  exit 1
fi
docker run --rm --network host \
  -e "MC_HOST_trpg=http://$minio_access_key:$minio_secret_key@127.0.0.1:19000" \
  "$minio_client_image" \
  mb --ignore-existing "trpg/$minio_bucket"

if [[ -n "${TRPG_POSTGRES_BINDIR:-}" ]]; then
  postgres_bindir="$TRPG_POSTGRES_BINDIR"
else
  postgres_bindir="$(pg_config --bindir)"
fi
pg_dump_path="$postgres_bindir/pg_dump"
pg_restore_path="$postgres_bindir/pg_restore"
for postgres_program in "$pg_dump_path" "$pg_restore_path"; do
  if [[ ! -f "$postgres_program" || -L "$postgres_program" ]]; then
    printf 'PostgreSQL tool must be a regular non-symlink file: %s\n' "$postgres_program" >&2
    exit 1
  fi
done
server_major="$(
  docker exec trpg-primary-postgres postgres --version |
    sed -E 's/.* ([0-9]+)([.].*)?$/\1/'
)"
client_major="$("$pg_dump_path" --version | sed -E 's/.* ([0-9]+)([.].*)?$/\1/')"
if [[ "$client_major" != "$server_major" ]]; then
  printf 'pg_dump major %s does not match PostgreSQL server major %s\n' \
    "$client_major" "$server_major" >&2
  exit 1
fi

for migration in "$root"/migrations/*.sql; do
  if [[ "$migration" == *.down.sql ]]; then
    continue
  fi
  docker exec -i trpg-primary-postgres \
    psql -X -v ON_ERROR_STOP=1 -1 -U postgres -d trpg_backup_source \
    <"$migration" >/dev/null
done

{
  printf '[trpg_backup_source]\n'
  printf 'host=127.0.0.1\nport=15432\nuser=postgres\npassword=%s\ndbname=trpg_backup_source\nsslmode=disable\n\n' \
    "$postgres_password"
  printf '[trpg_backup_target]\n'
  printf 'host=127.0.0.1\nport=15432\nuser=postgres\npassword=%s\ndbname=trpg_backup_target\nsslmode=disable\n' \
    "$postgres_password"
} >"$libpq_service_file"
chmod 0600 "$libpq_service_file"

cat >>"$github_env" <<ENVIRONMENT
P02_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_identity
P02_REDIS_URL=redis://127.0.0.1:16379
P02_CANONICAL_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_canonical
P02_CANONICAL_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p02_canonical_witness
P02_CANONICAL_ALLOW_DATABASE_RESET=1
P02_CANONICAL_RESET_DATABASE=p02_canonical
P02_CANONICAL_WITNESS_RESET_DATABASE=p02_canonical_witness
P02_EVENTING_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_eventing
P02_EVENTING_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p02_eventing_witness
P02_EVENTING_ALLOW_DATABASE_RESET=1
P02_EVENTING_RESET_DATABASE=p02_eventing
P02_EVENTING_WITNESS_RESET_DATABASE=p02_eventing_witness
P02_WORKFLOW_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_workflow
P02_API_CANONICAL_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_api_replay
P02_API_CANONICAL_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p02_api_replay_witness
P02_FORMAL_COMMIT_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_formal_commit
P02_FORMAL_COMMIT_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p02_formal_commit_witness
P02_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p02_service_witness
P02_NATS_URL=nats://127.0.0.1:14222
P03_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p03_migration_upgrade
P03_ALLOW_DATABASE_RESET=1
P04_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p04_eventing
P04_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p04_eventing_witness
P04_ALLOW_DATABASE_RESET=1
P04_ADMIN_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/postgres
P04_RECOVERY_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p04_eventing_recovery
P05_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p05_privacy
P05_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p05_privacy_witness
P06_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p06_core_domain
P06_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p06_core_domain_witness
P06_ALLOW_DATABASE_RESET=1
P06_RESET_DATABASE=p06_core_domain
P06_WITNESS_RESET_DATABASE=p06_core_domain_witness
P07_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p07_player_action
P07_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p07_player_action_witness
P07_ALLOW_DATABASE_RESET=1
P07_RESET_DATABASE=p07_player_action
P07_WITNESS_RESET_DATABASE=p07_player_action_witness
P07_NATS_URL=nats://127.0.0.1:14222
P05_REDIS_URL=redis://127.0.0.1:16379
P05_NATS_URL=nats://127.0.0.1:14222
P05_MINIO_ENDPOINT=http://127.0.0.1:19000
P05_MINIO_REGION=us-east-1
P05_MINIO_BUCKET=trpg-ci-deletion
ENVIRONMENT

{
  printf 'P02_TLS_DATABASE_URL=postgresql://postgres:%s@%s:15434/p02_tls_identity?sslmode=require\n' \
    "$postgres_password" "$tls_hostname"
  printf 'P02_TLS_CA_CERT_PATH=%s\n' "$tls_directory/ca.crt"
  printf 'P02_PG_DUMP=%s\n' "$pg_dump_path"
  printf 'P02_PG_RESTORE=%s\n' "$pg_restore_path"
  printf 'P02_LIBPQ_SERVICE_FILE=%s\n' "$libpq_service_file"
  printf 'P02_BACKUP_SOURCE_SERVICE=trpg_backup_source\n'
  printf 'P02_BACKUP_TARGET_SERVICE=trpg_backup_target\n'
  printf 'P02_BACKUP_SOURCE_URL=postgresql://postgres:%s@127.0.0.1:15432/trpg_backup_source\n' \
    "$postgres_password"
  printf 'P02_BACKUP_TARGET_URL=postgresql://postgres:%s@127.0.0.1:15432/trpg_backup_target\n' \
    "$postgres_password"
  printf 'P02_BACKUP_DIR=%s\n' "$backup_directory/artifacts"
  printf 'P04_PG_DUMP=%s\n' "$pg_dump_path"
  printf 'P04_PG_RESTORE=%s\n' "$pg_restore_path"
  printf 'P05_MINIO_ACCESS_KEY=%s\n' "$minio_access_key"
  printf 'P05_MINIO_SECRET_KEY=%s\n' "$minio_secret_key"
} >>"$github_env"

python3 "$root/scripts/ci/p02_policy_bootstrap.py" --github-env "$github_env"
