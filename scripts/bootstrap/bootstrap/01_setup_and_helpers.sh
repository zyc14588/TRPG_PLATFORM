#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'
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

root="$bootstrap_repository_root"
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
case "$provider_type" in
  cloud|cloud-provider|openai|anthropic) runtime_provider_type=cloud ;;
  ollama) runtime_provider_type=ollama ;;
  llama_cpp|llama.cpp) runtime_provider_type=llama_cpp ;;
  *) printf 'bootstrap error=PROVIDER_TYPE_INVALID\n' >&2; exit 2 ;;
esac
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
scratch="$runtime/bootstrap-scratch"
[[ ! -L "$scratch" && ( ! -e "$scratch" || -d "$scratch" ) ]] || {
  printf 'bootstrap error=SCRATCH_PATH_INVALID\n' >&2
  exit 4
}
install -d -m 0700 "$scratch"
find "$scratch" -mindepth 1 -depth -delete
cleanup() {
  local code="$?"
  find "$scratch" -mindepth 1 -depth -delete 2>/dev/null || true
  rmdir "$scratch" 2>/dev/null || true
  return "$code"
}
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
  payload_encryption_key audit_hmac_key local_model_certification_hmac_key
  admin_bootstrap_token redis_url nats_url
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
    local_model_certification_hmac_key admin_bootstrap_token realtime_cache_key; do
    random_secret "$name" 32
  done
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

tutorial_file="$credentials/tutorial.env"
