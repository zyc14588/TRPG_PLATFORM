#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

root="$service_process_smoke_repository_root"
release_dir="${CARGO_TARGET_DIR:-$root/target}/release"
temporary_directory="$(mktemp -d)"
secret_mount="$temporary_directory/secrets"
secret_catalog_directory="$temporary_directory/secret-catalog"
export_root="$temporary_directory/exports"
admin_root="$temporary_directory/admin"
admin_backup_directory="$admin_root/backups"
admin_safety_directory="$admin_root/restore-safety-points"
admin_certification_directory="$admin_root/model-certification-requests"
admin_certification_status_directory="$admin_root/model-certification-status"
admin_state_path="$admin_root/control-state.json"
admin_audit_path="$admin_root/audit.jsonl"
admin_openfga_store_id_path="$admin_root/openfga-store-id"
admin_openfga_model_id_path="$admin_root/openfga-model-id"
pids=()
pid_labels=()

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
admin_psql_path="${TRPG_ADMIN_PSQL_PATH:-${P02_PSQL:-}}"
admin_pg_dump_path="${TRPG_ADMIN_PG_DUMP_PATH:-${P02_PG_DUMP:-}}"
admin_pg_restore_path="${TRPG_ADMIN_PG_RESTORE_PATH:-${P02_PG_RESTORE:-}}"
admin_pg_service_file="${TRPG_ADMIN_PG_SERVICE_FILE_PATH:-${P02_LIBPQ_SERVICE_FILE:-}}"
admin_provider_ca_path="${TRPG_ADMIN_PROVIDER_CA_PATH:-/etc/ssl/certs/ca-certificates.crt}"

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
require_configuration "TRPG_ADMIN_PSQL_PATH or P02_PSQL" "$admin_psql_path"
require_configuration "TRPG_ADMIN_PG_DUMP_PATH or P02_PG_DUMP" "$admin_pg_dump_path"
require_configuration "TRPG_ADMIN_PG_RESTORE_PATH or P02_PG_RESTORE" "$admin_pg_restore_path"
require_configuration \
  "TRPG_ADMIN_PG_SERVICE_FILE_PATH or P02_LIBPQ_SERVICE_FILE" \
  "$admin_pg_service_file"
require_configuration "TRPG_ADMIN_PROVIDER_CA_PATH" "$admin_provider_ca_path"

identity_signing_key="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
audit_hmac_key="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
admin_bootstrap_token="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
canonical_hmac_key="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
payload_encryption_key="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
redis_cache_key="$(python3 -c 'import secrets; print(secrets.token_hex(16))')"
provider_credential="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
plugin_registry="$temporary_directory/plugin-registry.json"
printf '%s\n' '{"fuel_limit":100000,"memory_limit_bytes":1048576,"plugins":[]}' >"$plugin_registry"

install -d -m 0700 \
  "$secret_mount" "$secret_catalog_directory" "$export_root" \
  "$admin_root" "$admin_backup_directory" "$admin_safety_directory" \
  "$admin_certification_directory" "$admin_certification_status_directory"
(
  umask 077
  printf '%s\n' "$api_openfga_store_id" >"$admin_openfga_store_id_path"
  printf '%s\n' "$api_openfga_model_id" >"$admin_openfga_model_id_path"
)

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
write_secret admin_bootstrap_token "$admin_bootstrap_token"
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
  if [[ "$service" == api-server || "$service" == realtime-server ||
        "$service" == agent-worker || "$service" == admin-server ||
        "$service" == migration-runner ]]; then
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
      "TRPG_AGENT_WORKER_MODE=ready"
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
      "TRPG_LOCAL_PROVIDER_ENDPOINT_ALLOWLIST=loopback"
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
  if [[ "$service" == admin-server ]]; then
    command_environment+=(
      "TRPG_REDIS_URL_SECRET_ID=redis_url"
      "TRPG_REDIS_URL_SECRET_VERSION=1"
      "TRPG_IDENTITY_SIGNING_KEY_SECRET_ID=identity_signing_key"
      "TRPG_IDENTITY_SIGNING_KEY_SECRET_VERSION=1"
      "TRPG_ADMIN_BOOTSTRAP_TOKEN_SECRET_ID=admin_bootstrap_token"
      "TRPG_ADMIN_BOOTSTRAP_TOKEN_SECRET_VERSION=1"
      "TRPG_AUDIT_HMAC_KEY_ID=service-process-smoke-admin-v1"
      "TRPG_AUDIT_HMAC_KEY_SECRET_ID=audit_hmac_key"
      "TRPG_AUDIT_HMAC_KEY_SECRET_VERSION=1"
      "TRPG_ADMIN_STATE_PATH=$admin_state_path"
      "TRPG_ADMIN_AUDIT_LOG_PATH=$admin_audit_path"
      "TRPG_ADMIN_CURL_PATH=/usr/bin/curl"
      "TRPG_ADMIN_PSQL_PATH=$admin_psql_path"
      "TRPG_ADMIN_PROVIDER_CA_PATH=$admin_provider_ca_path"
      "TRPG_ADMIN_PG_DUMP_PATH=$admin_pg_dump_path"
      "TRPG_ADMIN_PG_RESTORE_PATH=$admin_pg_restore_path"
      "TRPG_ADMIN_PG_SERVICE_FILE_PATH=$admin_pg_service_file"
      "TRPG_ADMIN_OPENFGA_ADDRESS=$api_openfga_address"
      "TRPG_ADMIN_OPENFGA_STORE_ID_FILE=$admin_openfga_store_id_path"
      "TRPG_ADMIN_OPENFGA_MODEL_ID_FILE=$admin_openfga_model_id_path"
      "TRPG_ADMIN_BACKUP_SOURCE_SERVICE=trpg_backup_source"
      "TRPG_ADMIN_RESTORE_TARGET_SERVICE=trpg_backup_target"
      "TRPG_ADMIN_BACKUP_DIRECTORY=$admin_backup_directory"
      "TRPG_ADMIN_SAFETY_DIRECTORY=$admin_safety_directory"
      "TRPG_ADMIN_CERTIFICATION_DIRECTORY=$admin_certification_directory"
      "TRPG_ADMIN_CERTIFICATION_STATUS_DIRECTORY=$admin_certification_status_directory"
    )
  fi
  env "${command_environment[@]}" \
    "$binary" >"$temporary_directory/$service.log" 2>&1 &
  pids+=("$!")
  pid_labels+=("$service")
}
