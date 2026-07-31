#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
release_dir="${CARGO_TARGET_DIR:-$root/target}/release"
temporary_directory="$(mktemp -d)"
secret_mount="$temporary_directory/secrets"
secret_catalog_directory="$temporary_directory/secret-catalog"
export_root="$temporary_directory/exports"
pids=()

cleanup() {
  local pid
  for pid in "${pids[@]:-}"; do
    kill -TERM "$pid" 2>/dev/null || true
  done
  for pid in "${pids[@]:-}"; do
    wait "$pid" 2>/dev/null || true
  done
  rm -rf "$temporary_directory"
}
trap cleanup EXIT

require_configuration() {
  local description="$1"
  local value="$2"
  if [[ -z "$value" ]]; then
    printf 'service process smoke requires %s\n' "$description" >&2
    exit 1
  fi
}

api_database_url="${TRPG_DATABASE_URL:-${P02_DATABASE_URL:-}}"
worker_database_url="${TRPG_WORKER_DATABASE_URL:-${P02_WORKER_SERVICE_DATABASE_URL:-}}"
canonical_database_url="${TRPG_CANONICAL_DATABASE_URL:-${P02_CANONICAL_SERVICE_DATABASE_URL:-}}"
worker_witness_database_url="${TRPG_WORKER_WITNESS_DATABASE_URL:-${P02_WITNESS_APPEND_DATABASE_URL:-}}"
api_openfga_address="${TRPG_OPENFGA_ADDRESS:-${P02_OPENFGA_ADDRESS:-}}"
api_openfga_store_id="${TRPG_OPENFGA_STORE_ID:-${P02_OPENFGA_STORE_ID:-}}"
api_openfga_model_id="${TRPG_OPENFGA_MODEL_ID:-${P02_OPENFGA_MODEL_ID:-}}"
api_opa_address="${TRPG_OPA_ADDRESS:-${P02_OPA_ADDRESS:-}}"
api_opa_revision="${TRPG_OPA_POLICY_REVISION:-${P02_OPA_REVISION:-}}"
canonical_witness_url="${TRPG_WITNESS_DATABASE_URL:-${P02_WITNESS_DATABASE_URL:-}}"
nats_url="${TRPG_NATS_URL:-${P02_NATS_URL:-}}"
redis_url="${TRPG_REDIS_URL:-${P02_REDIS_URL:-}}"
object_storage_endpoint="${TRPG_OBJECT_STORAGE_ENDPOINT:-${P05_MINIO_ENDPOINT:-}}"
object_storage_region="${TRPG_OBJECT_STORAGE_REGION:-${P05_MINIO_REGION:-}}"
object_storage_bucket="${TRPG_OBJECT_STORAGE_BUCKET:-${P05_MINIO_BUCKET:-}}"
object_storage_ca_cert_path="${TRPG_OBJECT_STORAGE_CA_CERT_PATH:-${P05_MINIO_CA_CERT_PATH:-}}"
object_storage_access_key="${TRPG_OBJECT_STORAGE_ACCESS_KEY:-${P05_MINIO_ACCESS_KEY:-}}"
object_storage_secret_key="${TRPG_OBJECT_STORAGE_SECRET_KEY:-${P05_MINIO_SECRET_KEY:-}}"

require_configuration "TRPG_DATABASE_URL or P02_DATABASE_URL" "$api_database_url"
require_configuration \
  "TRPG_WORKER_DATABASE_URL or P02_WORKER_SERVICE_DATABASE_URL" \
  "$worker_database_url"
require_configuration \
  "TRPG_CANONICAL_DATABASE_URL or P02_CANONICAL_SERVICE_DATABASE_URL" \
  "$canonical_database_url"
require_configuration \
  "TRPG_WORKER_WITNESS_DATABASE_URL or P02_WITNESS_APPEND_DATABASE_URL" \
  "$worker_witness_database_url"
require_configuration "TRPG_OPENFGA_ADDRESS or P02_OPENFGA_ADDRESS" "$api_openfga_address"
require_configuration "TRPG_OPENFGA_STORE_ID or P02_OPENFGA_STORE_ID" "$api_openfga_store_id"
require_configuration "TRPG_OPENFGA_MODEL_ID or P02_OPENFGA_MODEL_ID" "$api_openfga_model_id"
require_configuration "TRPG_OPA_ADDRESS or P02_OPA_ADDRESS" "$api_opa_address"
require_configuration "TRPG_OPA_POLICY_REVISION or P02_OPA_REVISION" "$api_opa_revision"
require_configuration "TRPG_WITNESS_DATABASE_URL or P02_WITNESS_DATABASE_URL" "$canonical_witness_url"
require_configuration "TRPG_NATS_URL or P02_NATS_URL" "$nats_url"
require_configuration "TRPG_REDIS_URL or P02_REDIS_URL" "$redis_url"
require_configuration "TRPG_OBJECT_STORAGE_ENDPOINT or P05_MINIO_ENDPOINT" "$object_storage_endpoint"
require_configuration "TRPG_OBJECT_STORAGE_REGION or P05_MINIO_REGION" "$object_storage_region"
require_configuration "TRPG_OBJECT_STORAGE_BUCKET or P05_MINIO_BUCKET" "$object_storage_bucket"
require_configuration \
  "TRPG_OBJECT_STORAGE_CA_CERT_PATH or P05_MINIO_CA_CERT_PATH" \
  "$object_storage_ca_cert_path"
require_configuration \
  "TRPG_OBJECT_STORAGE_ACCESS_KEY or P05_MINIO_ACCESS_KEY" \
  "$object_storage_access_key"
require_configuration \
  "TRPG_OBJECT_STORAGE_SECRET_KEY or P05_MINIO_SECRET_KEY" \
  "$object_storage_secret_key"

identity_signing_key="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
audit_hmac_key="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
canonical_hmac_key="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
payload_encryption_key="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
redis_cache_key="$(python3 -c 'import secrets; print(secrets.token_hex(16))')"
provider_credential="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
plugin_registry="$temporary_directory/plugin-registry.json"
printf '%s\n' '{"fuel_limit":100000,"memory_limit_bytes":1048576,"plugins":[]}' >"$plugin_registry"

install -d -m 0700 "$secret_mount" "$secret_catalog_directory" "$export_root"

write_secret() {
  local secret_id="$1"
  local secret_value="$2"
  (
    umask 077
    printf '%s' "$secret_value" >"$secret_mount/$secret_id.v1"
  )
}

write_secret database_url "$api_database_url"
write_secret worker_database_url "$worker_database_url"
write_secret canonical_database_url "$canonical_database_url"
write_secret worker_witness_database_url "$worker_witness_database_url"
write_secret witness_database_url "$canonical_witness_url"
write_secret nats_url "$nats_url"
write_secret redis_url "$redis_url"
write_secret identity_signing_key "$identity_signing_key"
write_secret audit_hmac_key "$audit_hmac_key"
write_secret canonical_hmac_key "$canonical_hmac_key"
write_secret payload_encryption_key "$payload_encryption_key"
write_secret redis_cache_key "$redis_cache_key"
write_secret provider_credential "$provider_credential"
write_secret object_storage_access_key "$object_storage_access_key"
write_secret object_storage_secret_key "$object_storage_secret_key"

services=(api-server realtime-server agent-worker admin-server migration-runner)
environment_keys=(
  TRPG_API_SERVER_BIND
  TRPG_REALTIME_SERVER_BIND
  TRPG_AGENT_WORKER_BIND
  TRPG_ADMIN_SERVER_BIND
  TRPG_MIGRATION_RUNNER_BIND
)
component_checks=(
  api_runtime
  realtime_runtime
  agent_worker_runtime
  admin_runtime
  migration_runtime
)
ports=(18100 18101 18102 18103 18104)

start_service() {
  local index="$1"
  local service="${services[$index]}"
  local binary
  local database_secret_id="database_url"
  local secret_catalog_path
  local witness_database_secret_id="witness_database_url"
  local -a command_environment
  binary="$release_dir/$service"
  # Compose gives every process an independent state volume; preserve that catalog/witness pairing.
  secret_catalog_path="$secret_catalog_directory/$service/catalog.jsonl"
  install -d -m 0700 "$secret_catalog_directory/$service"
  test -x "$binary"
  if [[ "$service" == agent-worker ]]; then
    database_secret_id="worker_database_url"
    witness_database_secret_id="worker_witness_database_url"
  fi
  command_environment=("${environment_keys[$index]}=127.0.0.1:${ports[$index]}")
  if [[ "$service" == api-server || "$service" == realtime-server || "$service" == agent-worker || "$service" == migration-runner ]]; then
    command_environment+=(
      "TRPG_SECRET_MOUNT=$secret_mount"
      "TRPG_SECRET_CATALOG_PATH=$secret_catalog_path"
      "TRPG_DATABASE_URL_SECRET_ID=$database_secret_id"
      "TRPG_DATABASE_URL_SECRET_VERSION=1"
      "TRPG_PAYLOAD_ENCRYPTION_KEY_ID=service-process-smoke-payload-v1"
      "TRPG_PAYLOAD_ENCRYPTION_KEY_SECRET_ID=payload_encryption_key"
      "TRPG_PAYLOAD_ENCRYPTION_KEY_SECRET_VERSION=1"
      "TRPG_WITNESS_DATABASE_URL_SECRET_ID=$witness_database_secret_id"
      "TRPG_WITNESS_DATABASE_URL_SECRET_VERSION=1"
      "TRPG_CANONICAL_HMAC_KEY_ID=service-process-smoke-v1"
      "TRPG_CANONICAL_HMAC_KEY_SECRET_ID=canonical_hmac_key"
      "TRPG_CANONICAL_HMAC_KEY_SECRET_VERSION=1"
    )
  fi
  if [[ "$service" == realtime-server || "$service" == agent-worker ]]; then
    command_environment+=(
      "TRPG_NATS_URL_SECRET_ID=nats_url"
      "TRPG_NATS_URL_SECRET_VERSION=1"
      "TRPG_REDIS_URL_SECRET_ID=redis_url"
      "TRPG_REDIS_URL_SECRET_VERSION=1"
    )
  fi
  if [[ "$service" == realtime-server ]]; then
    command_environment+=(
      "TRPG_REDIS_CACHE_KEY_ID=redis_cache_key"
      "TRPG_REDIS_CACHE_KEY_VERSION=1"
    )
  fi
  if [[ "$service" == agent-worker ]]; then
    command_environment+=(
      "TRPG_CANONICAL_DATABASE_URL_SECRET_ID=canonical_database_url"
      "TRPG_CANONICAL_DATABASE_URL_SECRET_VERSION=1"
      "TRPG_IDENTITY_SIGNING_KEY_SECRET_ID=identity_signing_key"
      "TRPG_IDENTITY_SIGNING_KEY_SECRET_VERSION=1"
      "TRPG_OPENFGA_ADDRESS=$api_openfga_address"
      "TRPG_OPENFGA_STORE_ID=$api_openfga_store_id"
      "TRPG_OPENFGA_MODEL_ID=$api_openfga_model_id"
      "TRPG_OPA_ADDRESS=$api_opa_address"
      "TRPG_OPA_POLICY_REVISION=$api_opa_revision"
      "TRPG_AUDIT_LOG_PATH=$temporary_directory/agent-audit.jsonl"
      "TRPG_AUDIT_HMAC_KEY_ID=service-process-smoke-v1"
      "TRPG_AUDIT_HMAC_KEY_SECRET_ID=audit_hmac_key"
      "TRPG_AUDIT_HMAC_KEY_SECRET_VERSION=1"
      "TRPG_MODEL_PROVIDER_TYPE=cloud"
      "TRPG_MODEL_PROVIDER_ID=service-process-smoke-provider"
      "TRPG_MODEL_ID=service-process-smoke-model"
      "TRPG_MODEL_ARTIFACT_SHA256=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
      "TRPG_MODEL_PROVIDER_BASE_URL=https://models.example.invalid/v1"
      "TRPG_MODEL_PROVIDER_CREDENTIAL_SECRET_ID=provider_credential"
      "TRPG_MODEL_PROVIDER_CREDENTIAL_SECRET_VERSION=1"
      "TRPG_MODEL_ROUTE_AUTHORIZATION_EVENT_ID=service-process-smoke-route"
      "TRPG_MODEL_PROVIDER_CAPABILITIES=chat,streaming,structured_output,tool_requests,embeddings"
      "TRPG_MODEL_PROVIDER_TIMEOUT_MS=30000"
      "TRPG_PLUGIN_REGISTRY_PATH=$plugin_registry"
      "TRPG_OBJECT_STORAGE_ENDPOINT=$object_storage_endpoint"
      "TRPG_OBJECT_STORAGE_REGION=$object_storage_region"
      "TRPG_OBJECT_STORAGE_BUCKET=$object_storage_bucket"
      "TRPG_OBJECT_STORAGE_CA_CERT_PATH=$object_storage_ca_cert_path"
      "SSL_CERT_FILE=${SSL_CERT_FILE:-$object_storage_ca_cert_path}"
      "TRPG_OBJECT_STORAGE_ACCESS_KEY_SECRET_ID=object_storage_access_key"
      "TRPG_OBJECT_STORAGE_ACCESS_KEY_SECRET_VERSION=1"
      "TRPG_OBJECT_STORAGE_SECRET_KEY_SECRET_ID=object_storage_secret_key"
      "TRPG_OBJECT_STORAGE_SECRET_KEY_SECRET_VERSION=1"
      "TRPG_EXPORT_STORAGE_ROOT=$export_root"
    )
  fi
  if [[ "$service" == api-server ]]; then
    command_environment+=(
      "TRPG_CANONICAL_DATABASE_URL_SECRET_ID=database_url"
      "TRPG_CANONICAL_DATABASE_URL_SECRET_VERSION=1"
      "TRPG_IDENTITY_SIGNING_KEY_SECRET_ID=identity_signing_key"
      "TRPG_IDENTITY_SIGNING_KEY_SECRET_VERSION=1"
      "TRPG_REDIS_URL_SECRET_ID=redis_url"
      "TRPG_REDIS_URL_SECRET_VERSION=1"
      "TRPG_OPENFGA_ADDRESS=$api_openfga_address"
      "TRPG_OPENFGA_STORE_ID=$api_openfga_store_id"
      "TRPG_OPENFGA_MODEL_ID=$api_openfga_model_id"
      "TRPG_OPA_ADDRESS=$api_opa_address"
      "TRPG_OPA_POLICY_REVISION=$api_opa_revision"
      "TRPG_AUDIT_LOG_PATH=$temporary_directory/api-audit.jsonl"
      "TRPG_AUDIT_HMAC_KEY_ID=service-process-smoke-v1"
      "TRPG_AUDIT_HMAC_KEY_SECRET_ID=audit_hmac_key"
      "TRPG_AUDIT_HMAC_KEY_SECRET_VERSION=1"
    )
  fi
  env "${command_environment[@]}" \
    "$binary" >"$temporary_directory/$service.log" 2>&1 &
  pids+=("$!")
}

# Schema ownership is explicit: the migration runner must finish its startup
# migration/recovery gate before traffic-serving processes initialize their
# own persistent adapters.
migration_index=4
start_service "$migration_index"
migration_bootstrap_ready=false
for _ in $(seq 1 100); do
  if curl -fsS "http://127.0.0.1:${ports[$migration_index]}/health/ready" >/dev/null; then
    migration_bootstrap_ready=true
    break
  fi
  sleep 0.05
done
if [[ "$migration_bootstrap_ready" != true ]]; then
  cat "$temporary_directory/${services[$migration_index]}.log" >&2
  exit 1
fi

for index in "${!services[@]}"; do
  if [[ "$index" == "$migration_index" ]]; then
    continue
  fi
  start_service "$index"
done

for index in "${!services[@]}"; do
  service="${services[$index]}"
  port="${ports[$index]}"
  ready_document="$temporary_directory/$service-ready.json"
  live_document="$temporary_directory/$service-live.json"
  ready=false
  for _ in $(seq 1 100); do
    if curl -fsS "http://127.0.0.1:$port/health/ready" -o "$ready_document"; then
      ready=true
      break
    fi
    sleep 0.05
  done
  if [[ "$ready" != true ]]; then
    cat "$temporary_directory/$service.log" >&2
    exit 1
  fi
  curl -fsS "http://127.0.0.1:$port/health/live" -o "$live_document"

  python3 - "$port" <<'PY'
import socket
import sys

with socket.create_connection(("127.0.0.1", int(sys.argv[1])), timeout=2) as connection:
    connection.shutdown(socket.SHUT_WR)
PY

  survived_eof=false
  for _ in $(seq 1 40); do
    if curl -fsS "http://127.0.0.1:$port/health/live" -o "$live_document"; then
      survived_eof=true
      break
    fi
    sleep 0.05
  done
  if [[ "$survived_eof" != true ]]; then
    cat "$temporary_directory/$service.log" >&2
    exit 1
  fi

  python3 - "$service" "${component_checks[$index]}" "$ready_document" "$live_document" <<'PY'
import json
import sys
from pathlib import Path

service, role_check, ready_path, live_path = sys.argv[1:]
ready = json.loads(Path(ready_path).read_text(encoding="utf-8"))
live = json.loads(Path(live_path).read_text(encoding="utf-8"))
assert ready["service"] == service
assert ready["status"] == "ready"
assert ready["state"] == "ready"
assert ready["version"]
checks = {check["name"]: check for check in ready["checks"]}
assert {"configuration", "event_registry", "listener", role_check} <= checks.keys()
assert all(check["status"] == "pass" and check["detail"] for check in checks.values())
assert live == {
    "service": service,
    "state": "ready",
    "status": "live",
    "version": ready["version"],
}
PY

  status="$({ curl -sS -o "$temporary_directory/$service-not-found.json" -w '%{http_code}' \
    "http://127.0.0.1:$port/not-a-health-route"; } 2>/dev/null)"
  test "$status" = 404
  grep -q '"error":"NOT_FOUND"' "$temporary_directory/$service-not-found.json"
done

web_pid=""
node "$root/apps/web/scripts/serve.mjs" --root dist --port 18105 \
  >"$temporary_directory/web.log" 2>&1 &
web_pid="$!"
pids+=("$web_pid")
web_ready=false
for _ in $(seq 1 100); do
  if curl -fsS "http://127.0.0.1:18105/" -o "$temporary_directory/web.html"; then
    web_ready=true
    break
  fi
  sleep 0.05
done
if [[ "$web_ready" != true ]]; then
  cat "$temporary_directory/web.log" >&2
  exit 1
fi
grep -q '<h1>COC AI TRPG</h1>' "$temporary_directory/web.html"
curl -fsS "http://127.0.0.1:18105/config.json" -o "$temporary_directory/web-config.json"
python3 - "$temporary_directory/web-config.json" <<'PY'
import json
import sys
from pathlib import Path
from urllib.parse import urlparse

configuration = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
services = configuration["services"]
assert len(services) == 5
urls = [urlparse(service["url"]) for service in services]
assert all(url.scheme == "http" and url.hostname == "127.0.0.1" for url in urls)
assert {url.port for url in urls} == {8080, 8081, 8082, 8083, 8084}
PY

for pid in "${pids[@]}"; do
  kill -TERM "$pid"
done
for pid in "${pids[@]}"; do
  wait "$pid"
done
pids=()

printf 'service process smoke: 5 services and web passed\n'
