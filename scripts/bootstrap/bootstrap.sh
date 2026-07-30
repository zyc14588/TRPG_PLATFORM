#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

usage() {
  cat <<'USAGE'
Secure, resumable COC AI TRPG bootstrap (Bash 5+):
  bootstrap.sh --state-dir /absolute/private/state --project-name unique-project
    --provider-type openai --provider-url https://provider.example/v1
    --provider-model exact-model-id --provider-sha256 sha256:<64-hex>
    --provider-credential-file /absolute/private/token
    [--provider-ca-file /absolute/ca.pem] [--extra-compose-file /absolute/compose.yml]
Re-running resumes completed steps. Secrets are written only to the reported
private credentials file; they are never printed.
USAGE
}

state_dir='' project='' provider_type='' provider_url='' provider_model=''
provider_sha256='' provider_credential_file='' provider_ca_file='/etc/ssl/certs/ca-certificates.crt'
extra_compose_file=''
while (($#)); do
  case "$1" in
    --state-dir) state_dir="${2:-}"; shift 2 ;;
    --project-name) project="${2:-}"; shift 2 ;;
    --provider-type) provider_type="${2:-}"; shift 2 ;;
    --provider-url) provider_url="${2:-}"; shift 2 ;;
    --provider-model) provider_model="${2:-}"; shift 2 ;;
    --provider-sha256) provider_sha256="${2:-}"; shift 2 ;;
    --provider-credential-file) provider_credential_file="${2:-}"; shift 2 ;;
    --provider-ca-file) provider_ca_file="${2:-}"; shift 2 ;;
    --extra-compose-file) extra_compose_file="${2:-}"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) printf 'bootstrap error=UNKNOWN_ARGUMENT argument=%q\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
((BASH_VERSINFO[0] >= 5)) || { printf 'bootstrap error=BASH_5_REQUIRED\n' >&2; exit 2; }
for command_name in docker openssl curl python3 install flock sha256sum sync; do
  command -v "$command_name" >/dev/null 2>&1 || { printf 'bootstrap error=COMMAND_REQUIRED command=%s\n' "$command_name" >&2; exit 2; }
done
docker compose version >/dev/null 2>&1 || { printf 'bootstrap error=DOCKER_COMPOSE_V2_REQUIRED\n' >&2; exit 2; }
[[ "$state_dir" = /* && "$project" =~ ^[a-z0-9][a-z0-9_-]{2,62}$ ]] || { printf 'bootstrap error=STATE_OR_PROJECT_INVALID\n' >&2; exit 2; }
[[ "$provider_sha256" =~ ^sha256:[0-9a-fA-F]{64}$ ]] || { printf 'bootstrap error=PROVIDER_DIGEST_INVALID\n' >&2; exit 2; }
for value in "$provider_type" "$provider_url" "$provider_model"; do
  [[ -n "$value" && ${#value} -le 256 ]] || { printf 'bootstrap error=PROVIDER_CONFIGURATION_INVALID\n' >&2; exit 2; }
done
if ! python3 - "$provider_url" <<'PY'; then
import sys
from urllib.parse import urlsplit
u = urlsplit(sys.argv[1])
if u.scheme != "https" or not u.hostname or u.username or u.password or u.query or u.fragment:
    raise SystemExit("provider URL must be credential-free HTTPS")
PY
  printf 'bootstrap error=PROVIDER_URL_INVALID\n' >&2; exit 2
fi
for path in "$provider_credential_file" "$provider_ca_file"; do
  [[ "$path" = /* && -f "$path" && ! -L "$path" ]] || { printf 'bootstrap error=PROVIDER_FILE_INVALID\n' >&2; exit 2; }
done
extra_digest='none'
if [[ -n "$extra_compose_file" ]]; then
  [[ "$extra_compose_file" = /* && -f "$extra_compose_file" && ! -L "$extra_compose_file" ]] || { printf 'bootstrap error=EXTRA_COMPOSE_FILE_INVALID\n' >&2; exit 2; }
  extra_digest="$(sha256sum "$extra_compose_file" | awk '{print $1}')"
fi

install -d -m 0700 "$state_dir"
[[ ! -L "$state_dir" ]] || { printf 'bootstrap error=STATE_SYMLINK_FORBIDDEN\n' >&2; exit 2; }
exec 9>"$state_dir/bootstrap.lock"
flock -n 9 || { printf 'bootstrap error=BOOTSTRAP_ALREADY_RUNNING\n' >&2; exit 75; }
runtime="$state_dir/runtime" secrets="$runtime/secrets" credentials="$state_dir/credentials"
requests="$runtime/requests" journal="$state_dir/state.tsv"
install -d -m 0700 "$runtime" "$secrets" "$credentials" "$requests"
scratch="$(mktemp -d)"
cleanup() { local code="$?"; rm -rf "$scratch"; return "$code"; }
trap cleanup EXIT

atomic_text() {
  local target="$1" mode="$2" temporary
  temporary="$target.tmp.$$"
  cat >"$temporary"
  chmod "$mode" "$temporary"
  mv "$temporary" "$target"
  sync -f "$target"
}
if [[ ! -f "$journal" ]]; then
  printf '0\tbootstrap.initialize\tREADY\n' | atomic_text "$journal" 0600
fi
step_done() { awk -F $'\t' -v step="$1" '$2 == step && $3 == "OK" {found=1} END {exit !found}' "$journal"; }
commit_step() {
  local step="$1" version temporary="$journal.tmp.$$"
  version="$(awk -F $'\t' 'END {print $1 + 1}' "$journal")"
  { cat "$journal"; printf '%s\t%s\tOK\n' "$version" "$step"; } >"$temporary"
  chmod 0600 "$temporary"; mv "$temporary" "$journal"; sync -f "$journal"
  printf 'bootstrap step=%s result=OK version=%s\n' "$step" "$version"
  [[ "${TRPG_BOOTSTRAP_TEST_STOP_AFTER_STEP:-}" != "$step" ]] || kill -STOP "$$"
}

configuration="$state_dir/configuration.tsv"
printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
  "$project" "$provider_type" "$provider_url" "$provider_model" "${provider_sha256,,}" "$extra_digest" \
  >"$scratch/configuration"
if [[ -f "$configuration" ]]; then
  cmp -s "$configuration" "$scratch/configuration" || { printf 'bootstrap error=CONFIGURATION_CHANGED_FORK_REQUIRED\n' >&2; exit 3; }
else
  atomic_text "$configuration" 0600 <"$scratch/configuration"
fi
step_done preflight || commit_step preflight

secret_names=(
  postgres_bootstrap_password postgres_witness_owner_password
  postgres_witness_append_password postgres_witness_read_password
  postgres_api_password postgres_canonical_password postgres_worker_password
  postgres_realtime_password postgres_backup_password postgres_restore_password
  owner_database_url api_database_url canonical_database_url worker_database_url
  realtime_database_url witness_owner_database_url witness_append_database_url
  witness_read_database_url identity_signing_key canonical_hmac_key
  payload_encryption_key audit_hmac_key admin_bootstrap_token redis_url nats_url
  realtime_cache_key object_storage_access_key object_storage_secret_key
  redis_acl redis_healthcheck_password nats_authorization minio_root_user
  minio_root_password postgres_tls_certificate postgres_tls_private_key
  postgres_ca_certificate redis_tls_certificate redis_tls_private_key
  redis_tls_ca_certificate redis_client_tls_certificate
  redis_client_tls_private_key redis_healthcheck_tls_certificate
  redis_healthcheck_tls_private_key nats_tls_certificate nats_tls_private_key
  nats_tls_ca_certificate nats_client_tls_certificate
  nats_client_tls_private_key minio_tls_certificate minio_tls_private_key
  minio_tls_ca_certificate reverse_proxy_tls_certificate
  reverse_proxy_tls_private_key provider_credential provider_ca_certificate
  admin_pg_service_file admin_pg_passfile
)
write_secret() {
  local name="$1" value="$2"
  [[ -f "$secrets/$name" && ! -L "$secrets/$name" ]] && return 0
  printf '%s' "$value" | atomic_text "$secrets/$name" 0444
}
random_secret() {
  local name="$1" bytes="${2:-24}"
  [[ -f "$secrets/$name" && ! -L "$secrets/$name" ]] && return 0
  openssl rand -hex "$bytes" | tr -d '\n' | atomic_text "$secrets/$name" 0444
}
issue_certificate() {
  local prefix="$1" common_name="$2" usage="$3" alternatives="$4" serial="$5"
  local key="$runtime/$prefix.key" cert="$runtime/$prefix.crt" csr="$scratch/$prefix.csr"
  [[ ! -f "$cert" || -f "$key" ]] || { printf 'bootstrap error=CERTIFICATE_KEY_MISSING certificate=%s\n' "$prefix" >&2; exit 4; }
  [[ -f "$key" ]] || openssl genrsa -out "$key" 2048 >/dev/null 2>&1
  if [[ ! -f "$cert" ]]; then
    openssl req -new -sha256 -key "$key" -subj "/CN=$common_name" -out "$csr" >/dev/null 2>&1
    printf 'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=%s\nsubjectAltName=%s\n' \
      "$usage" "$alternatives" >"$scratch/$prefix.ext"
    openssl x509 -req -sha256 -days 825 -in "$csr" -CA "$runtime/ca.crt" \
      -CAkey "$runtime/ca.key" -set_serial "$serial" -extfile "$scratch/$prefix.ext" \
      -out "$cert" >/dev/null 2>&1
  fi
  chmod 0600 "$key"; chmod 0644 "$cert"
}
copy_secret() {
  local name="$1" source="$2"
  [[ -f "$secrets/$name" && ! -L "$secrets/$name" ]] || install -m 0444 "$source" "$secrets/$name"
}

if ! step_done secrets; then
  if [[ -f "$runtime/ca.crt" && ! -f "$runtime/ca.key" ]]; then
    printf 'bootstrap error=CA_KEY_MISSING\n' >&2; exit 4
  fi
  [[ -f "$runtime/ca.key" ]] || openssl genrsa -out "$runtime/ca.key" 3072 >/dev/null 2>&1
  [[ -f "$runtime/ca.crt" ]] || openssl req -x509 -new -sha256 -days 3650 \
    -key "$runtime/ca.key" -subj "/CN=COC AI TRPG Private Root" -out "$runtime/ca.crt" >/dev/null 2>&1
  issue_certificate postgres_server postgres serverAuth \
    "DNS:postgres,DNS:postgres-witness,DNS:localhost,IP:127.0.0.1" 1001
  issue_certificate redis_server redis serverAuth "DNS:redis,DNS:localhost,IP:127.0.0.1" 1002
  issue_certificate nats_server nats serverAuth "DNS:nats,DNS:localhost,IP:127.0.0.1" 1003
  issue_certificate minio_server minio serverAuth "DNS:minio,DNS:localhost,IP:127.0.0.1" 1004
  issue_certificate reverse_proxy reverse-proxy serverAuth \
    "DNS:reverse-proxy,DNS:localhost,IP:127.0.0.1" 1005
  issue_certificate redis_client redis-client clientAuth "DNS:redis-client" 1006
  issue_certificate redis_healthcheck redis-healthcheck clientAuth "DNS:redis-healthcheck" 1007
  issue_certificate nats_client nats-client clientAuth "DNS:nats-client" 1008
  for name in postgres_bootstrap_password postgres_witness_owner_password \
    postgres_witness_append_password postgres_witness_read_password postgres_api_password \
    postgres_canonical_password postgres_worker_password postgres_realtime_password \
    postgres_backup_password postgres_restore_password redis_healthcheck_password \
    redis_application_password nats_password minio_root_password minio_service_password; do
    random_secret "$name"
  done
  for name in identity_signing_key canonical_hmac_key payload_encryption_key audit_hmac_key \
    admin_bootstrap_token realtime_cache_key; do random_secret "$name" 32; done
  pgo="$(<"$secrets/postgres_bootstrap_password")" pgw="$(<"$secrets/postgres_witness_owner_password")"
  pga="$(<"$secrets/postgres_api_password")" pgc="$(<"$secrets/postgres_canonical_password")"
  pgwk="$(<"$secrets/postgres_worker_password")" pgr="$(<"$secrets/postgres_realtime_password")"
  wpa="$(<"$secrets/postgres_witness_append_password")" wpr="$(<"$secrets/postgres_witness_read_password")"
  rbp="$(<"$secrets/postgres_backup_password")" rrp="$(<"$secrets/postgres_restore_password")"
  redis_password="$(<"$secrets/redis_application_password")" nats_password="$(<"$secrets/nats_password")"
  write_secret owner_database_url "postgresql://trpg_database_owner:$pgo@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
  write_secret api_database_url "postgresql://trpg_api_login:$pga@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
  write_secret canonical_database_url "postgresql://trpg_canonical_login:$pgc@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
  write_secret worker_database_url "postgresql://trpg_worker_login:$pgwk@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
  write_secret realtime_database_url "postgresql://trpg_realtime_login:$pgr@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
  write_secret witness_owner_database_url "postgresql://trpg_witness_owner:$pgw@postgres-witness:5432/coc_ai_trpg_witness?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
  write_secret witness_append_database_url "postgresql://trpg_witness_append_login:$wpa@postgres-witness:5432/coc_ai_trpg_witness?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
  write_secret witness_read_database_url "postgresql://trpg_witness_read_login:$wpr@postgres-witness:5432/coc_ai_trpg_witness?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
  write_secret redis_url "rediss://trpg_runtime:$redis_password@redis:6379"
  write_secret nats_url "tls://runtime:$nats_password@nats:4222"
  write_secret object_storage_access_key "trpg_s3_${project//-/_}"
  write_secret object_storage_secret_key "$(<"$secrets/minio_service_password")"
  write_secret minio_root_user "trpg_root_${project//-/_}"
  rhp="$(<"$secrets/redis_healthcheck_password")"
  write_secret redis_acl "user default off
user healthcheck on >$rhp ~* +ping
user trpg_runtime on >$redis_password ~* +@all"
  write_secret nats_authorization "authorization {
  users = [{ user: \"runtime\", password: \"$nats_password\" }]
}"
  write_secret admin_pg_service_file "[trpg_backup_source]
host=postgres
port=5432
dbname=coc_ai_trpg
user=trpg_backup_login
sslmode=verify-full
sslrootcert=/run/secrets/postgres_ca_certificate
[trpg_restore_target]
host=postgres
port=5432
dbname=coc_ai_trpg_restore
user=trpg_restore_login
sslmode=verify-full
sslrootcert=/run/secrets/postgres_ca_certificate"
  write_secret admin_pg_passfile "postgres:5432:coc_ai_trpg:trpg_backup_login:$rbp
postgres:5432:coc_ai_trpg_restore:trpg_restore_login:$rrp"
  copy_secret provider_credential "$provider_credential_file"; copy_secret provider_ca_certificate "$provider_ca_file"
  for pair in postgres_tls_certificate:postgres_server.crt postgres_tls_private_key:postgres_server.key \
    postgres_ca_certificate:ca.crt redis_tls_certificate:redis_server.crt \
    redis_tls_private_key:redis_server.key redis_tls_ca_certificate:ca.crt \
    redis_client_tls_certificate:redis_client.crt redis_client_tls_private_key:redis_client.key \
    redis_healthcheck_tls_certificate:redis_healthcheck.crt \
    redis_healthcheck_tls_private_key:redis_healthcheck.key nats_tls_certificate:nats_server.crt \
    nats_tls_private_key:nats_server.key nats_tls_ca_certificate:ca.crt \
    nats_client_tls_certificate:nats_client.crt nats_client_tls_private_key:nats_client.key \
    minio_tls_certificate:minio_server.crt minio_tls_private_key:minio_server.key \
    minio_tls_ca_certificate:ca.crt reverse_proxy_tls_certificate:reverse_proxy.crt \
    reverse_proxy_tls_private_key:reverse_proxy.key; do
    copy_secret "${pair%%:*}" "$runtime/${pair#*:}"
  done
  credentials_file="$credentials/initial-accounts.env"
  if [[ ! -f "$credentials_file" ]]; then
    {
      printf 'ADMIN_USER_ID=%q\n' "server-owner-$project"
      printf 'ADMIN_LOGIN=%q\n' "owner@$project.invalid"
      printf 'ADMIN_PASSWORD=%q\n' "$(openssl rand -hex 24)"
      printf 'BUSINESS_USER_ID=%q\n' "business-user-$project"
      printf 'BUSINESS_LOGIN=%q\n' "business@$project.invalid"
      printf 'BUSINESS_PASSWORD=%q\n' "$(openssl rand -hex 24)"
    } | atomic_text "$credentials_file" 0600
  fi
  commit_step secrets
fi

overlay="$runtime/compose.bootstrap.yml"
if ! step_done compose_config; then
  {
    printf 'secrets:\n'
    for name in "${secret_names[@]}"; do
      [[ -s "$secrets/$name" && ! -L "$secrets/$name" ]] || { printf 'bootstrap error=SECRET_MISSING name=%s\n' "$name" >&2; exit 4; }
      printf '  %s:\n    external: false\n    file: %s/%s\n' "$name" "$secrets" "$name"
    done
  } | atomic_text "$overlay" 0600
  commit_step compose_config
fi
export TRPG_CANONICAL_HMAC_KEY_ID="$project-canonical-v1"
export TRPG_PAYLOAD_ENCRYPTION_KEY_ID="$project-payload-v1"
export TRPG_AUDIT_HMAC_KEY_ID="$project-audit-v1"
export TRPG_OBJECT_STORAGE_BUCKET="trpg-$project"
compose=(docker compose --project-name "$project" -f "$root/compose.yml")
[[ -z "$extra_compose_file" ]] || compose+=(-f "$extra_compose_file")
compose+=(-f "$overlay")
"${compose[@]}" config --quiet
step_done compose_up || { "${compose[@]}" up --detach --build --wait --wait-timeout 300; commit_step compose_up; }

if ! step_done recovery_roles; then
  "${compose[@]}" exec -T postgres sh -ec \
    'export PGPASSWORD="$(cat /run/secrets/postgres_bootstrap_password)"; exec psql -X -q -v ON_ERROR_STOP=1 -U trpg_database_owner -d postgres' <<'SQL'
SELECT 'CREATE ROLE trpg_backup_login LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS'
 WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_backup_login') \gexec
SELECT format('ALTER ROLE trpg_backup_login PASSWORD %L', btrim(pg_read_file('/run/secrets/postgres_backup_password'))) \gexec
SELECT 'CREATE ROLE trpg_restore_login LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS'
 WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_restore_login') \gexec
SELECT format('ALTER ROLE trpg_restore_login PASSWORD %L', btrim(pg_read_file('/run/secrets/postgres_restore_password'))) \gexec
SELECT 'CREATE DATABASE coc_ai_trpg_restore OWNER trpg_restore_login'
 WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'coc_ai_trpg_restore') \gexec
REVOKE ALL ON DATABASE coc_ai_trpg_restore FROM PUBLIC;
GRANT CONNECT ON DATABASE coc_ai_trpg TO trpg_backup_login;
GRANT CONNECT ON DATABASE coc_ai_trpg_restore TO trpg_restore_login;
\connect coc_ai_trpg_restore
CREATE EXTENSION IF NOT EXISTS vector;
REVOKE ALL ON SCHEMA public FROM PUBLIC;
GRANT USAGE, CREATE ON SCHEMA public TO trpg_restore_login;
\connect coc_ai_trpg
GRANT USAGE ON SCHEMA public, core_domain TO trpg_backup_login;
GRANT SELECT ON ALL TABLES IN SCHEMA public, core_domain TO trpg_backup_login;
GRANT SELECT ON ALL SEQUENCES IN SCHEMA public, core_domain TO trpg_backup_login;
ALTER DEFAULT PRIVILEGES FOR ROLE trpg_database_owner IN SCHEMA public GRANT SELECT ON TABLES TO trpg_backup_login;
ALTER DEFAULT PRIVILEGES FOR ROLE trpg_database_owner IN SCHEMA core_domain GRANT SELECT ON TABLES TO trpg_backup_login;
SQL
  commit_step recovery_roles
fi

# shellcheck disable=SC1090
source "$credentials/initial-accounts.env"
api_base='https://127.0.0.1:8443/admin/v1'
api_call() {
  local method="$1" path="$2" headers="$3" body="$4" output="$5"
  local -a arguments=(--silent --show-error --cacert "$runtime/ca.crt" --request "$method" --header "@$headers" --output "$output" --write-out '%{http_code}')
  [[ -z "$body" ]] || arguments+=(--header 'Content-Type: application/json' --data-binary "@$body")
  curl "${arguments[@]}" "$api_base/$path"
}
json_value() { python3 - "$1" "$2" <<'PY'
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
for part in sys.argv[2].split("."):
    value = value[part]
print(str(value).lower() if isinstance(value, bool) else value)
PY
}
login_body="$scratch/login.json" complete_body="$scratch/complete.json"
python3 - "$credentials/initial-accounts.env" "$login_body" "$complete_body" <<'PY'
import json, shlex, sys
values = {}
for line in open(sys.argv[1], encoding="utf-8"):
    key, value = line.rstrip("\n").split("=", 1)
    values[key] = shlex.split(value)[0]
json.dump({"login": values["ADMIN_LOGIN"], "password": values["ADMIN_PASSWORD"]}, open(sys.argv[2], "w"))
json.dump({"administrator": {"user_id": values["ADMIN_USER_ID"], "login": values["ADMIN_LOGIN"], "password": values["ADMIN_PASSWORD"]},
           "business_account": {"user_id": values["BUSINESS_USER_ID"], "login": values["BUSINESS_LOGIN"], "password": values["BUSINESS_PASSWORD"]}}, open(sys.argv[3], "w"))
PY
owner_headers="$scratch/owner.headers" response="$scratch/response.json"
owner_login() {
  printf 'Accept: application/json\n' >"$scratch/public.headers"
  [[ "$(api_call POST sessions "$scratch/public.headers" "$login_body" "$response")" == 200 ]] || return 1
  printf 'Authorization: Bearer %s\nAccept: application/json\n' "$(json_value "$response" access_token)" >"$owner_headers"
}
if ! step_done admin_bootstrap; then
  if ! owner_login; then
    printf 'Authorization: Bearer %s\nAccept: application/json\n' "$(<"$secrets/admin_bootstrap_token")" >"$scratch/bootstrap.headers"
    [[ "$(api_call GET bootstrap/status "$scratch/bootstrap.headers" '' "$response")" == 200 ]] || { printf 'bootstrap error=BOOTSTRAP_STATUS_FAILED\n' >&2; exit 5; }
    version="$(json_value "$response" state_version)"
    {
      cat "$scratch/bootstrap.headers"
      printf 'Idempotency-Key: %s-bootstrap\nX-Expected-Version: %s\nX-Correlation-Id: %s-bootstrap\nX-Causation-Id: %s-compose\n' "$project" "$version" "$project" "$project"
    } >"$scratch/mutation.headers"
    [[ "$(api_call POST bootstrap/complete "$scratch/mutation.headers" "$complete_body" "$response")" == 201 ]] || { printf 'bootstrap error=ADMIN_BOOTSTRAP_FAILED\n' >&2; exit 5; }
    owner_login || { printf 'bootstrap error=ADMIN_LOGIN_FAILED\n' >&2; exit 5; }
  fi
  commit_step admin_bootstrap
else
  owner_login || { printf 'bootstrap error=ADMIN_LOGIN_FAILED\n' >&2; exit 5; }
fi

mutation_headers() {
  local version="$1" key="$2"
  { cat "$owner_headers"; printf 'Idempotency-Key: %s\nX-Expected-Version: %s\nX-Correlation-Id: %s\nX-Causation-Id: %s-bootstrap\n' "$key" "$version" "$key" "$project"; } >"$scratch/mutation.headers"
}
if ! step_done provider_configure; then
  [[ "$(api_call GET bootstrap/status "$owner_headers" '' "$response")" == 200 ]]
  if [[ "$(json_value "$response" provider_configured)" != true ]]; then
    version="$(json_value "$response" state_version)"
    python3 - "$provider_type" "$provider_url" "$provider_model" "${provider_sha256,,}" "$scratch/provider.json" <<'PY'
import json, sys
json.dump({"provider_type": sys.argv[1], "base_url": sys.argv[2], "model_id": sys.argv[3],
           "model_artifact_sha256": sys.argv[4], "credential_secret_id": "provider_credential",
           "credential_secret_version": 1}, open(sys.argv[5], "w"))
PY
    mutation_headers "$version" "$project-provider-configure"
    [[ "$(api_call PUT providers/configuration "$scratch/mutation.headers" "$scratch/provider.json" "$response")" == 200 ]] || { printf 'bootstrap error=PROVIDER_CONFIGURATION_FAILED\n' >&2; exit 6; }
  fi
  commit_step provider_configure
fi
if ! step_done provider_probe; then
  [[ "$(api_call GET bootstrap/status "$owner_headers" '' "$response")" == 200 ]]
  mutation_headers "$(json_value "$response" state_version)" "$project-provider-probe"
  [[ "$(api_call POST providers/probe "$scratch/mutation.headers" '' "$response")" == 200 ]] || { printf 'bootstrap error=PROVIDER_PROBE_FAILED\n' >&2; exit 6; }
  commit_step provider_probe
fi
if ! step_done model_certification; then
  [[ "$(api_call GET bootstrap/status "$owner_headers" '' "$response")" == 200 ]]
  mutation_headers "$(json_value "$response" state_version)" "$project-model-certification"
  python3 - "$project" "$provider_model" "${provider_sha256,,}" "$scratch/certification.json" <<'PY'
import json, sys
json.dump({"request_id": sys.argv[1] + "-model-certification", "model_id": sys.argv[2],
           "model_artifact_sha256": sys.argv[3]}, open(sys.argv[4], "w"))
PY
  [[ "$(api_call POST models/certification-requests "$scratch/mutation.headers" "$scratch/certification.json" "$response")" == 200 ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_REQUEST_FAILED\n' >&2; exit 6; }
  commit_step model_certification
fi
if ! step_done self_check; then
  [[ "$(api_call GET diagnostics "$owner_headers" '' "$response")" == 200 ]]
  [[ "$(api_call GET audit "$owner_headers" '' "$response")" == 200 ]]
  curl --fail --silent --show-error --cacert "$runtime/ca.crt" https://127.0.0.1:8443/ >/dev/null
  commit_step self_check
fi
printf 'bootstrap complete state=%s credentials=%s endpoint=https://127.0.0.1:8443\n' \
  "$journal" "$credentials/initial-accounts.env"
