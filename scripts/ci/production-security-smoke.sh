#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
runtime_directory="$(mktemp -d)"
secret_directory="$runtime_directory/secrets"
project_name="trpg-security-${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}"
swarm_service="${project_name}-secret-canary"
swarm_secret_v1="${project_name}-certificate-v1"
swarm_secret_v2="${project_name}-certificate-v2"
initialized_swarm=false
compose_command=(
  docker compose
  --project-name "$project_name"
  -f "$root/compose.yml"
  -f "$root/docker-compose.ci.yml"
)

cleanup() {
  local exit_code="$?"
  if [[ "$exit_code" -ne 0 ]]; then
    printf 'production security smoke failed; Compose diagnostics follow\n' >&2
    "${compose_command[@]}" ps --all >&2 || true
    "${compose_command[@]}" logs --no-color --tail 200 >&2 || true
  fi
  "${compose_command[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
  docker service rm "$swarm_service" >/dev/null 2>&1 || true
  docker secret rm "$swarm_secret_v1" "$swarm_secret_v2" >/dev/null 2>&1 || true
  if [[ "$initialized_swarm" == true ]]; then
    docker swarm leave --force >/dev/null 2>&1 || true
  fi
  rm -rf "$runtime_directory"
  return "$exit_code"
}
trap cleanup EXIT

if [[ "${GITHUB_ACTIONS:-}" != true ]]; then
  printf 'production security smoke requires an isolated GitHub Actions Docker daemon\n' >&2
  exit 1
fi

for required_command in \
  docker openssl psql curl python3 timeout sha256sum awk; do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    printf 'production security smoke requires command: %s\n' "$required_command" >&2
    exit 1
  fi
done

install -d -m 0700 "$secret_directory"
certificate_serial=1000

issue_certificate() {
  local prefix="$1"
  local common_name="$2"
  local extended_usage="$3"
  local subject_alternative_name="$4"
  certificate_serial=$((certificate_serial + 1))
  openssl req -newkey rsa:2048 -nodes -sha256 \
    -subj "/CN=$common_name" \
    -keyout "$runtime_directory/$prefix.key" \
    -out "$runtime_directory/$prefix.csr" >/dev/null 2>&1
  {
    printf 'basicConstraints=critical,CA:FALSE\n'
    printf 'keyUsage=critical,digitalSignature,keyEncipherment\n'
    printf 'extendedKeyUsage=%s\n' "$extended_usage"
    printf 'subjectAltName=%s\n' "$subject_alternative_name"
  } >"$runtime_directory/$prefix.ext"
  openssl x509 -req -sha256 -days 1 \
    -in "$runtime_directory/$prefix.csr" \
    -CA "$runtime_directory/ca.crt" \
    -CAkey "$runtime_directory/ca.key" \
    -set_serial "$certificate_serial" \
    -extfile "$runtime_directory/$prefix.ext" \
    -out "$runtime_directory/$prefix.crt" >/dev/null 2>&1
  chmod 0600 "$runtime_directory/$prefix.key"
  chmod 0644 "$runtime_directory/$prefix.crt"
}

write_secret() {
  local name="$1"
  local value="$2"
  (
    umask 077
    printf '%s' "$value" >"$secret_directory/$name"
  )
}

copy_secret() {
  local name="$1"
  local source="$2"
  install -m 0600 "$source" "$secret_directory/$name"
}

witness_query() {
  local role="$1"
  local password="$2"
  local statement="$3"
  PGPASSWORD="$password" \
    PGSSLMODE=verify-full \
    PGSSLROOTCERT="$runtime_directory/ca.crt" \
    psql -X -A -t --set=ON_ERROR_STOP=1 \
      -h localhost -p 25433 -U "$role" -d coc_ai_trpg_witness \
      -c "$statement"
}

expect_witness_denied() {
  local role="$1"
  local password="$2"
  local operation="$3"
  local statement="$4"
  if witness_query "$role" "$password" "$statement" >/dev/null 2>&1; then
    printf 'PostgreSQL witness runtime role unexpectedly allowed %s: %s\n' \
      "$operation" "$role" >&2
    exit 1
  fi
}

wait_for_task_container() {
  local service_name="$1"
  local previous_container="${2:-}"
  local attempt
  local -a container_ids=()
  for ((attempt = 1; attempt <= 120; attempt++)); do
    mapfile -t container_ids < <(
      docker ps \
        --filter "label=com.docker.swarm.service.name=$service_name" \
        --filter status=running \
        --format '{{.ID}}'
    )
    if [[ "${#container_ids[@]}" -eq 1 &&
      "${container_ids[0]}" != "$previous_container" ]]; then
      printf '%s\n' "${container_ids[0]}"
      return 0
    fi
    sleep 0.5
  done
  printf 'timed out waiting for one running task container for %s\n' "$service_name" >&2
  docker service ps --no-trunc "$service_name" >&2 || true
  docker ps --all \
    --filter "label=com.docker.swarm.service.name=$service_name" >&2 || true
  return 1
}

openssl req -x509 -newkey rsa:3072 -nodes -sha256 -days 1 \
  -subj "/CN=TRPG Production Security Smoke Root" \
  -keyout "$runtime_directory/ca.key" \
  -out "$runtime_directory/ca.crt" >/dev/null 2>&1
chmod 0600 "$runtime_directory/ca.key"
chmod 0644 "$runtime_directory/ca.crt"

issue_certificate \
  postgres_server postgres serverAuth \
  "DNS:postgres,DNS:postgres-witness,DNS:localhost,IP:127.0.0.1"
issue_certificate redis_server redis serverAuth \
  "DNS:redis,DNS:localhost,IP:127.0.0.1"
issue_certificate nats_server nats serverAuth \
  "DNS:nats,DNS:localhost,IP:127.0.0.1"
issue_certificate minio_server_v1 minio serverAuth \
  "DNS:minio,DNS:localhost,IP:127.0.0.1"
issue_certificate reverse_proxy reverse-proxy serverAuth \
  "DNS:reverse-proxy,DNS:localhost,IP:127.0.0.1"
issue_certificate redis_client redis-client clientAuth "DNS:redis-client"
issue_certificate redis_healthcheck redis-healthcheck clientAuth "DNS:redis-healthcheck"
issue_certificate nats_client nats-client clientAuth "DNS:nats-client"

postgres_owner_password="$(openssl rand -hex 24)"
postgres_witness_owner_password="$(openssl rand -hex 24)"
postgres_witness_append_password="$(openssl rand -hex 24)"
postgres_witness_read_password="$(openssl rand -hex 24)"
postgres_api_password="$(openssl rand -hex 24)"
postgres_canonical_password="$(openssl rand -hex 24)"
postgres_worker_password="$(openssl rand -hex 24)"
postgres_realtime_password="$(openssl rand -hex 24)"
redis_healthcheck_password="$(openssl rand -hex 24)"
redis_application_password="$(openssl rand -hex 24)"
nats_password="$(openssl rand -hex 24)"
minio_user="trpg_runtime_smoke"
minio_password="$(openssl rand -hex 24)"

write_secret postgres_bootstrap_password "$postgres_owner_password"
write_secret postgres_witness_owner_password "$postgres_witness_owner_password"
write_secret postgres_witness_append_password "$postgres_witness_append_password"
write_secret postgres_witness_read_password "$postgres_witness_read_password"
write_secret postgres_api_password "$postgres_api_password"
write_secret postgres_canonical_password "$postgres_canonical_password"
write_secret postgres_worker_password "$postgres_worker_password"
write_secret postgres_realtime_password "$postgres_realtime_password"
write_secret owner_database_url \
  "postgresql://trpg_database_owner:$postgres_owner_password@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret api_database_url \
  "postgresql://trpg_api_login:$postgres_api_password@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret canonical_database_url \
  "postgresql://trpg_canonical_login:$postgres_canonical_password@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret worker_database_url \
  "postgresql://trpg_worker_login:$postgres_worker_password@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret realtime_database_url \
  "postgresql://trpg_realtime_login:$postgres_realtime_password@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret witness_owner_database_url \
  "postgresql://trpg_witness_owner:$postgres_witness_owner_password@postgres-witness:5432/coc_ai_trpg_witness?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret witness_append_database_url \
  "postgresql://trpg_witness_append_login:$postgres_witness_append_password@postgres-witness:5432/coc_ai_trpg_witness?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret witness_read_database_url \
  "postgresql://trpg_witness_read_login:$postgres_witness_read_password@postgres-witness:5432/coc_ai_trpg_witness?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret identity_signing_key "$(openssl rand -hex 32)"
write_secret canonical_hmac_key "$(openssl rand -hex 32)"
write_secret payload_encryption_key "$(openssl rand -hex 32)"
write_secret audit_hmac_key "$(openssl rand -hex 32)"
write_secret redis_url "rediss://trpg_runtime:$redis_application_password@redis:6379"
write_secret nats_url "tls://runtime_smoke:$nats_password@nats:4222"
write_secret realtime_cache_key "$(openssl rand -hex 32)"
write_secret object_storage_access_key "$minio_user"
write_secret object_storage_secret_key "$minio_password"
write_secret redis_healthcheck_password "$redis_healthcheck_password"
write_secret minio_root_user "$minio_user"
write_secret minio_root_password "$minio_password"
write_secret redis_acl \
  "user default off
user healthcheck on >$redis_healthcheck_password ~* +ping
user trpg_runtime on >$redis_application_password ~* +@all"
write_secret nats_authorization \
  "authorization {
  users = [
    { user: \"runtime_smoke\", password: \"$nats_password\" }
  ]
}"

copy_secret postgres_tls_certificate "$runtime_directory/postgres_server.crt"
copy_secret postgres_tls_private_key "$runtime_directory/postgres_server.key"
copy_secret postgres_ca_certificate "$runtime_directory/ca.crt"
copy_secret redis_tls_certificate "$runtime_directory/redis_server.crt"
copy_secret redis_tls_private_key "$runtime_directory/redis_server.key"
copy_secret redis_tls_ca_certificate "$runtime_directory/ca.crt"
copy_secret redis_client_tls_certificate "$runtime_directory/redis_client.crt"
copy_secret redis_client_tls_private_key "$runtime_directory/redis_client.key"
copy_secret redis_healthcheck_tls_certificate "$runtime_directory/redis_healthcheck.crt"
copy_secret redis_healthcheck_tls_private_key "$runtime_directory/redis_healthcheck.key"
copy_secret nats_tls_certificate "$runtime_directory/nats_server.crt"
copy_secret nats_tls_private_key "$runtime_directory/nats_server.key"
copy_secret nats_tls_ca_certificate "$runtime_directory/ca.crt"
copy_secret nats_client_tls_certificate "$runtime_directory/nats_client.crt"
copy_secret nats_client_tls_private_key "$runtime_directory/nats_client.key"
copy_secret minio_tls_certificate "$runtime_directory/minio_server_v1.crt"
copy_secret minio_tls_private_key "$runtime_directory/minio_server_v1.key"
copy_secret minio_tls_ca_certificate "$runtime_directory/ca.crt"
copy_secret reverse_proxy_tls_certificate "$runtime_directory/reverse_proxy.crt"
copy_secret reverse_proxy_tls_private_key "$runtime_directory/reverse_proxy.key"

# Compose file-backed secrets retain their source mode and ignore per-secret
# uid/gid/mode overrides. The parent remains 0700 on the isolated runner, while
# 0444 mirrors Docker-managed secret mounts and lets non-root service UIDs read
# only the files explicitly mounted into their containers.
chmod 0444 "$secret_directory"/*

export TRPG_COMPOSE_SECRET_DIRECTORY="$secret_directory"
export TRPG_CANONICAL_HMAC_KEY_ID="runtime-smoke-canonical-v1"
export TRPG_PAYLOAD_ENCRYPTION_KEY_ID="runtime-smoke-payload-v1"
export TRPG_AUDIT_HMAC_KEY_ID="runtime-smoke-audit-v1"
export TRPG_OBJECT_STORAGE_BUCKET="trpg-runtime-smoke"
export TRPG_OBJECT_STORAGE_REGION="us-east-1"
export TRPG_IMAGE_TAG="runtime-smoke"

python3 "$root/scripts/ci/verify_compose_security.py" --check
"${compose_command[@]}" config --quiet
"${compose_command[@]}" pull postgres postgres-witness redis nats minio minio-init
"${compose_command[@]}" up --detach --wait --wait-timeout 240 \
  postgres postgres-witness redis nats minio
"${compose_command[@]}" run --rm witness-role-bootstrap
"${compose_command[@]}" run --rm minio-init

if PGPASSWORD="$postgres_api_password" PGSSLMODE=disable \
  psql -X -h localhost -p 25432 -U trpg_api_login -d coc_ai_trpg \
  -c "SELECT 1" >/dev/null 2>&1; then
  printf 'PostgreSQL accepted a plaintext TCP connection\n' >&2
  exit 1
fi
postgres_tls="$(
  PGPASSWORD="$postgres_api_password" \
  PGSSLMODE=verify-full \
  PGSSLROOTCERT="$runtime_directory/ca.crt" \
  psql -X -A -t -h localhost -p 25432 \
    -U trpg_api_login -d coc_ai_trpg \
    -c "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()"
)"
if [[ "$postgres_tls" != t ]]; then
  printf 'PostgreSQL TLS verification did not reach an SSL session\n' >&2
  exit 1
fi

if PGPASSWORD="$postgres_witness_append_password" PGSSLMODE=disable \
  psql -X -h localhost -p 25433 -U trpg_witness_append_login -d coc_ai_trpg_witness \
  -c "SELECT 1" >/dev/null 2>&1; then
  printf 'PostgreSQL witness accepted a plaintext TCP connection\n' >&2
  exit 1
fi
witness_tls="$(
  PGPASSWORD="$postgres_witness_append_password" \
  PGSSLMODE=verify-full \
  PGSSLROOTCERT="$runtime_directory/ca.crt" \
  psql -X -A -t -h localhost -p 25433 \
    -U trpg_witness_append_login -d coc_ai_trpg_witness \
    -c "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()"
)"
if [[ "$witness_tls" != t ]]; then
  printf 'PostgreSQL witness TLS verification did not reach an SSL session\n' >&2
  exit 1
fi

redis_probe() {
  printf "*3\r\n\$4\r\nAUTH\r\n\$11\r\nhealthcheck\r\n\$%s\r\n%s\r\n*1\r\n\$4\r\nPING\r\n" \
    "${#redis_healthcheck_password}" "$redis_healthcheck_password" |
    timeout 10 openssl s_client "$@"
}

redis_without_client="$(
  redis_probe \
    -connect localhost:26379 \
    -servername localhost \
    -CAfile "$runtime_directory/ca.crt" \
    -verify_return_error \
    -quiet 2>/dev/null || true
)"
if [[ "$redis_without_client" == *PONG* ]]; then
  printf 'Redis accepted TLS without a client certificate\n' >&2
  exit 1
fi
redis_with_client="$(
  redis_probe \
    -connect localhost:26379 \
    -servername localhost \
    -CAfile "$runtime_directory/ca.crt" \
    -cert "$runtime_directory/redis_healthcheck.crt" \
    -key "$runtime_directory/redis_healthcheck.key" \
    -verify_return_error \
    -quiet 2>/dev/null || true
)"
if [[ "$redis_with_client" != *PONG* ]]; then
  printf 'Redis mTLS client did not receive PONG\n' >&2
  exit 1
fi

nats_request="CONNECT {\"user\":\"runtime_smoke\",\"pass\":\"$nats_password\",\"verbose\":false}"$'\r\nPING\r\n'
nats_without_client="$(
  printf '%s' "$nats_request" |
    timeout 10 openssl s_client \
      -connect localhost:24222 \
      -servername localhost \
      -CAfile "$runtime_directory/ca.crt" \
      -verify_return_error \
      -quiet 2>/dev/null || true
)"
if [[ "$nats_without_client" == *PONG* ]]; then
  printf 'NATS accepted TLS without a client certificate\n' >&2
  exit 1
fi
nats_with_client="$(
  printf '%s' "$nats_request" |
    timeout 10 openssl s_client \
      -connect localhost:24222 \
      -servername localhost \
      -CAfile "$runtime_directory/ca.crt" \
      -cert "$runtime_directory/nats_client.crt" \
      -key "$runtime_directory/nats_client.key" \
      -verify_return_error \
      -quiet 2>/dev/null || true
)"
if [[ "$nats_with_client" != *PONG* ]]; then
  printf 'NATS mTLS client did not receive PONG\n' >&2
  exit 1
fi

curl --fail --silent --show-error \
  --cacert "$runtime_directory/ca.crt" \
  https://localhost:29000/minio/health/live >/dev/null
if curl --fail --silent http://localhost:29000/minio/health/live >/dev/null 2>&1; then
  printf 'MinIO accepted plaintext HTTP on its TLS endpoint\n' >&2
  exit 1
fi

minio_fingerprint_v1="$(
  openssl s_client \
    -connect localhost:29000 \
    -servername localhost \
    -CAfile "$runtime_directory/ca.crt" </dev/null 2>/dev/null |
    openssl x509 -noout -fingerprint -sha256
)"
issue_certificate minio_server_v2 minio serverAuth \
  "DNS:minio,DNS:localhost,IP:127.0.0.1"
copy_secret minio_tls_certificate "$runtime_directory/minio_server_v2.crt"
copy_secret minio_tls_private_key "$runtime_directory/minio_server_v2.key"
"${compose_command[@]}" up --detach --wait --wait-timeout 180 --force-recreate minio
"${compose_command[@]}" run --rm minio-init
minio_fingerprint_v2="$(
  openssl s_client \
    -connect localhost:29000 \
    -servername localhost \
    -CAfile "$runtime_directory/ca.crt" </dev/null 2>/dev/null |
    openssl x509 -noout -fingerprint -sha256
)"
if [[ "$minio_fingerprint_v1" == "$minio_fingerprint_v2" ]]; then
  printf 'MinIO certificate fingerprint did not change after rotation\n' >&2
  exit 1
fi
curl --fail --silent --show-error \
  --cacert "$runtime_directory/ca.crt" \
  https://localhost:29000/minio/health/live >/dev/null

# All Rust services intentionally share one runtime image. Building every
# service in parallel asks Buildx to export the same tag five times and can
# race its snapshot extraction. Build each distinct target once, then require
# the full graph to start from exactly those local images.
"${compose_command[@]}" build api web
"${compose_command[@]}" up --detach --no-build --wait --wait-timeout 600

# Seed an existing-volume privilege regression, then require the idempotent
# bootstrap to remove every direct grant, dangerous role flag, role setting,
# and owner membership. The exact matrices below prove the repair took effect.
witness_query trpg_witness_owner "$postgres_witness_owner_password" "
GRANT CREATE, TEMPORARY ON DATABASE coc_ai_trpg_witness
    TO trpg_witness_read_login;
GRANT CREATE ON SCHEMA public TO trpg_witness_read_login;
GRANT UPDATE, DELETE, TRUNCATE ON TABLE external_audit_witness
    TO trpg_witness_append_login;
GRANT INSERT ON TABLE external_audit_witness TO trpg_witness_read_login;
GRANT trpg_witness_owner TO trpg_witness_append_login;
ALTER ROLE trpg_witness_append_login CREATEDB CREATEROLE BYPASSRLS;
ALTER ROLE trpg_witness_append_login SET search_path = pg_catalog;
" >/dev/null
"${compose_command[@]}" run --rm witness-role-bootstrap

append_privileges="$(
  witness_query trpg_witness_append_login "$postgres_witness_append_password" "
SELECT concat_ws(
    '|',
    current_user,
    has_database_privilege(current_user, current_database(), 'CONNECT'),
    has_database_privilege(current_user, current_database(), 'CREATE'),
    has_database_privilege(current_user, current_database(), 'TEMPORARY'),
    has_schema_privilege(current_user, 'public', 'USAGE'),
    has_schema_privilege(current_user, 'public', 'CREATE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'SELECT'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'INSERT'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'UPDATE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'DELETE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'TRUNCATE'),
    pg_has_role(current_user, 'trpg_witness_owner', 'MEMBER'),
    pg_has_role(current_user, 'trpg_witness_append_service', 'MEMBER'),
    pg_has_role(current_user, 'trpg_witness_read_service', 'MEMBER'),
    rolsuper,
    rolcreatedb,
    rolcreaterole,
    rolreplication,
    rolbypassrls
)
FROM pg_roles
WHERE rolname = current_user;
"
)"
expected_append_privileges="trpg_witness_append_login|t|f|f|t|f|t|t|f|f|f|f|t|f|f|f|f|f|f"
if [[ "$append_privileges" != "$expected_append_privileges" ]]; then
  printf 'PostgreSQL witness append privilege matrix is not least-privilege: %s\n' \
    "$append_privileges" >&2
  exit 1
fi

read_privileges="$(
  witness_query trpg_witness_read_login "$postgres_witness_read_password" "
SELECT concat_ws(
    '|',
    current_user,
    has_database_privilege(current_user, current_database(), 'CONNECT'),
    has_database_privilege(current_user, current_database(), 'CREATE'),
    has_database_privilege(current_user, current_database(), 'TEMPORARY'),
    has_schema_privilege(current_user, 'public', 'USAGE'),
    has_schema_privilege(current_user, 'public', 'CREATE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'SELECT'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'INSERT'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'UPDATE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'DELETE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'TRUNCATE'),
    pg_has_role(current_user, 'trpg_witness_owner', 'MEMBER'),
    pg_has_role(current_user, 'trpg_witness_append_service', 'MEMBER'),
    pg_has_role(current_user, 'trpg_witness_read_service', 'MEMBER'),
    rolsuper,
    rolcreatedb,
    rolcreaterole,
    rolreplication,
    rolbypassrls
)
FROM pg_roles
WHERE rolname = current_user;
"
)"
expected_read_privileges="trpg_witness_read_login|t|f|f|t|f|t|f|f|f|f|f|f|t|f|f|f|f|f"
if [[ "$read_privileges" != "$expected_read_privileges" ]]; then
  printf 'PostgreSQL witness read privilege matrix is not least-privilege: %s\n' \
    "$read_privileges" >&2
  exit 1
fi

witness_query trpg_witness_read_login "$postgres_witness_read_password" \
  "SELECT count(*) FROM external_audit_witness;" >/dev/null

witness_query trpg_witness_append_login "$postgres_witness_append_password" "
BEGIN;
INSERT INTO external_audit_witness (
    sequence,
    commit_id,
    phase,
    primary_request_hash,
    primary_first_sequence,
    primary_last_sequence,
    reason,
    integrity_key_id,
    previous_hash,
    record_hash
)
SELECT
    0,
    'runtime-privilege-probe-' || txid_current()::text,
    'PREPARED',
    'sha256:' || lpad(to_hex(txid_current()), 64, '0'),
    NULL,
    NULL,
    'least-privilege smoke probe',
    'runtime-privilege-probe',
    COALESCE(
        (
            SELECT record_hash
            FROM external_audit_witness
            ORDER BY sequence DESC
            LIMIT 1
        ),
        'hmac-sha256:' || repeat('0', 64)
    ),
    'hmac-sha256:' || lpad(to_hex(txid_current()), 64, '0');
ROLLBACK;
" >/dev/null

expect_witness_denied \
  trpg_witness_append_login "$postgres_witness_append_password" update \
  "BEGIN; UPDATE external_audit_witness SET reason = reason WHERE false; ROLLBACK;"
expect_witness_denied \
  trpg_witness_append_login "$postgres_witness_append_password" delete \
  "BEGIN; DELETE FROM external_audit_witness WHERE false; ROLLBACK;"
expect_witness_denied \
  trpg_witness_append_login "$postgres_witness_append_password" truncate \
  "BEGIN; TRUNCATE TABLE external_audit_witness; ROLLBACK;"
expect_witness_denied \
  trpg_witness_append_login "$postgres_witness_append_password" drop-table \
  "BEGIN; DROP TABLE external_audit_witness; ROLLBACK;"
expect_witness_denied \
  trpg_witness_append_login "$postgres_witness_append_password" create-table \
  "BEGIN; CREATE TABLE witness_privilege_escape_probe(id integer); ROLLBACK;"
expect_witness_denied \
  trpg_witness_read_login "$postgres_witness_read_password" insert \
  "
BEGIN;
INSERT INTO external_audit_witness (
    sequence,
    commit_id,
    phase,
    primary_request_hash,
    primary_first_sequence,
    primary_last_sequence,
    reason,
    integrity_key_id,
    previous_hash,
    record_hash
)
SELECT
    0,
    'read-privilege-escape-' || txid_current()::text,
    'PREPARED',
    'sha256:' || lpad(to_hex(txid_current()), 64, '0'),
    NULL,
    NULL,
    'read privilege escape probe',
    'runtime-privilege-probe',
    COALESCE(
        (
            SELECT record_hash
            FROM external_audit_witness
            ORDER BY sequence DESC
            LIMIT 1
        ),
        'hmac-sha256:' || repeat('0', 64)
    ),
    'hmac-sha256:' || lpad(to_hex(txid_current()), 64, '0');
ROLLBACK;
"

plaintext_proxy_status="$(
  curl --silent --output /dev/null \
    --write-out '%{http_code} %{redirect_url}' http://localhost:8080/ || true
)"
if [[ "$plaintext_proxy_status" != 308\ https://* ]]; then
  printf 'reverse proxy plaintext endpoint did not enforce the HTTPS redirect: %s\n' \
    "$plaintext_proxy_status" >&2
  exit 1
fi
if curl --fail --silent http://localhost:8443/ >/dev/null 2>&1; then
  printf 'reverse proxy TLS port accepted plaintext HTTP\n' >&2
  exit 1
fi
for runtime_path in \
  / \
  /api/health/ready \
  /realtime/health/ready \
  /admin/health/ready; do
  curl --fail --silent --show-error \
    --cacert "$runtime_directory/ca.crt" \
    "https://localhost:8443$runtime_path" >/dev/null
done

swarm_state="$(docker info --format '{{.Swarm.LocalNodeState}}')"
if [[ "$swarm_state" == inactive ]]; then
  docker swarm init --advertise-addr 127.0.0.1 >/dev/null
  initialized_swarm=true
elif [[ "$swarm_state" != active ]]; then
  printf 'Docker Swarm is neither inactive nor active: %s\n' "$swarm_state" >&2
  exit 1
fi
docker secret create "$swarm_secret_v1" "$runtime_directory/minio_server_v1.crt" >/dev/null
docker secret create "$swarm_secret_v2" "$runtime_directory/minio_server_v2.crt" >/dev/null
docker service create \
  --name "$swarm_service" \
  --constraint node.role==manager \
  --restart-condition none \
  --secret "source=$swarm_secret_v1,target=server.crt" \
  --entrypoint /bin/sh \
  nginx:1.27-alpine@sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10 \
  -c 'test -s /run/secrets/server.crt; while :; do sleep 30; done' >/dev/null

canary_container="$(wait_for_task_container "$swarm_service")"
expected_v1="$(sha256sum "$runtime_directory/minio_server_v1.crt" | awk '{print $1}')"
mounted_v1="$(docker exec "$canary_container" sha256sum /run/secrets/server.crt | awk '{print $1}')"
if [[ "$mounted_v1" != "$expected_v1" ]]; then
  printf 'Docker-managed external secret v1 was not mounted byte-for-byte\n' >&2
  exit 1
fi

docker service update \
  --secret-rm "$swarm_secret_v1" \
  --secret-add "source=$swarm_secret_v2,target=server.crt" \
  "$swarm_service" >/dev/null
rotated_container="$(wait_for_task_container "$swarm_service" "$canary_container")"
expected_v2="$(sha256sum "$runtime_directory/minio_server_v2.crt" | awk '{print $1}')"
mounted_v2="$(docker exec "$rotated_container" sha256sum /run/secrets/server.crt | awk '{print $1}')"
if [[ "$mounted_v2" != "$expected_v2" ]]; then
  printf 'Docker-managed external secret v2 was not mounted after rotation\n' >&2
  exit 1
fi
docker secret rm "$swarm_secret_v1" >/dev/null

printf 'production security smoke: full product graph, witness least privilege, TLS, Redis/NATS mTLS, certificate rotation, and external-secret rotation passed\n'
